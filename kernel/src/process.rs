//! SMP-safe ownership of Ring 3 processes and their scheduler reservations.
//!
//! A `Process` owns one private `AddressSpace`, one per-process file table and
//! at most one CPU-qualified `Ring3TaskHandle`. The process-table lock is never
//! held while taking a scheduler run-queue lock, so there is no lock-order
//! inversion. Reservation and publication are separate steps: a task is never
//! runnable until its full Ring 3 context has been built.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::{
    address_space::{AddressSpace, AddressSpaceError, cr3_root, read_cr3},
    file_descriptors::ProcessFileTable,
    rootfs,
    scheduler::{self, Ring3TaskContextRef, Ring3TaskHandle, SchedulerError, TaskId},
    sync::SpinLock,
};

pub const MAX_PROCESSES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProcessState {
    Prepared = 0,
    Publishing = 1,
    Runnable = 2,
    Running = 3,
    Exited = 4,
    /// Exited and queued; pins or an active CPU mask still guard the space.
    Reaping = 5,
    /// Resources destroyed; the descriptor slot may be reused.
    Reaped = 6,
}

/// Owner of the resources that make one Ring 3 process independent from every
/// other process on the system. The scheduler only stores an opaque context id;
/// it never owns or frees these resources.
pub struct Process {
    pid: u32,
    state: ProcessState,
    address_space: Option<AddressSpace>,
    entry: u64,
    user_stack: u64,
    task: Option<Ring3TaskHandle>,
    file_table: ProcessFileTable,
    exit_code: Option<u32>,
    /// Readers increment this before dereferencing a terminating process.
    /// The reaper only destroys once the count reaches zero.
    pin_count: AtomicUsize,
    /// CPUs whose CR3 currently references this process's private space.
    active_cpu_mask: AtomicU64,
    /// Set when reaping starts; prevents new pins and new activations.
    closing: AtomicBool,
}

impl Process {
    fn new(
        pid: u32,
        address_space: AddressSpace,
        entry: u64,
        user_stack: u64,
        file_table: ProcessFileTable,
        task: Option<Ring3TaskHandle>,
    ) -> Self {
        Self {
            pid,
            state: ProcessState::Prepared,
            address_space: Some(address_space),
            entry,
            user_stack,
            task,
            file_table,
            exit_code: None,
            pin_count: AtomicUsize::new(0),
            active_cpu_mask: AtomicU64::new(0),
            closing: AtomicBool::new(false),
        }
    }

    pub const fn pid(&self) -> u32 { self.pid }
    pub const fn state(&self) -> ProcessState { self.state }
    pub const fn entry(&self) -> u64 { self.entry }
    pub const fn user_stack(&self) -> u64 { self.user_stack }
    pub const fn task_handle(&self) -> Option<Ring3TaskHandle> { self.task }
    pub const fn exit_code(&self) -> Option<u32> { self.exit_code }

    pub fn address_space_root(&self) -> Option<u64> {
        self.address_space.as_ref().map(AddressSpace::root)
    }

    pub fn file_table(&self) -> &ProcessFileTable { &self.file_table }
    pub fn file_table_mut(&mut self) -> &mut ProcessFileTable { &mut self.file_table }
}

struct ProcessTable {
    slots: [Option<Process>; MAX_PROCESSES],
    next_pid: u32,
}

impl ProcessTable {
    const fn new() -> Self {
        Self {
            slots: [const { None }; MAX_PROCESSES],
            next_pid: 1,
        }
    }

    fn insert(
        &mut self,
        address_space: AddressSpace,
        entry: u64,
        user_stack: u64,
        file_table: ProcessFileTable,
        task: Option<Ring3TaskHandle>,
    ) -> Result<u32, ProcessError> {
        let slot = self
            .slots
            .iter()
            .position(Option::is_none)
            .ok_or(ProcessError::TableFull)?;
        let pid = self.next_pid;
        self.next_pid = pid.checked_add(1).ok_or(ProcessError::PidExhausted)?;
        if pid == 0 {
            return Err(ProcessError::PidExhausted);
        }
        self.slots[slot] = Some(Process::new(
            pid,
            address_space,
            entry,
            user_stack,
            file_table,
            task,
        ));
        Ok(pid)
    }

    fn get(&self, pid: u32) -> Option<&Process> {
        self.slots.iter().flatten().find(|process| process.pid == pid)
    }

    fn get_mut(&mut self, pid: u32) -> Option<&mut Process> {
        self.slots
            .iter_mut()
            .flatten()
            .find(|process| process.pid == pid)
    }

    fn remove(&mut self, pid: u32) -> Option<Process> {
        let slot = self
            .slots
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|process| process.pid == pid))?;
        self.slots[slot].take()
    }
}

static PROCESS_TABLE: SpinLock<ProcessTable> = SpinLock::new(ProcessTable::new());
static CURRENT_PID: [AtomicU32; scheduler::MAX_CPUS] =
    [const { AtomicU32::new(0) }; scheduler::MAX_CPUS];
/// Context ids are never recycled. A stale handle therefore cannot match a
/// later reservation of the same PID or the same scheduler slot.
static NEXT_CONTEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessError {
    TableFull,
    PidExhausted,
    NotFound,
    InvalidState,
    TaskAlreadyReserved,
    TaskNotReserved,
    WrongCpu,
    AlreadyExited,
    ActiveAddressSpace,
    Scheduler(SchedulerError),
    AddressSpace(AddressSpaceError),
}

fn next_context_ref() -> Ring3TaskContextRef {
    let id = NEXT_CONTEXT_ID.fetch_add(1, Ordering::AcqRel);
    // usize::MAX is the scheduler's "no context" sentinel.
    if id == usize::MAX {
        NEXT_CONTEXT_ID.store(1, Ordering::Release);
        Ring3TaskContextRef::new(1)
    } else {
        Ring3TaskContextRef::new(id)
    }
}

/// Insert a fully constructed address space as a non-runnable process.
pub fn create_prepared_process(
    address_space: AddressSpace,
    entry: u64,
    user_stack: u64,
    file_table: ProcessFileTable,
) -> Result<u32, ProcessError> {
    PROCESS_TABLE
        .lock()
        .insert(address_space, entry, user_stack, file_table, None)
}

/// Reserve the scheduler slot first, then insert the process that owns it.
/// Any table failure cancels the unpublished reservation before returning.
pub fn create_process_with_task(
    address_space: AddressSpace,
    entry: u64,
    user_stack: u64,
    file_table: ProcessFileTable,
) -> Result<(u32, Ring3TaskHandle), ProcessError> {
    let handle = scheduler::reserve_ring3_task(next_context_ref()).map_err(ProcessError::Scheduler)?;
    match PROCESS_TABLE.lock().insert(
        address_space,
        entry,
        user_stack,
        file_table,
        Some(handle),
    ) {
        Ok(pid) => Ok((pid, handle)),
        Err(error) => {
            let _ = scheduler::cancel_ring3_reservation(handle);
            Err(error)
        }
    }
}

/// Associate a new, non-recycled scheduler reservation with an existing process.
/// The process-table lock is released before the scheduler lock is taken.
pub fn reserve_process_task(pid: u32) -> Result<Ring3TaskHandle, ProcessError> {
    {
        let table = PROCESS_TABLE.lock();
        let process = table.get(pid).ok_or(ProcessError::NotFound)?;
        if process.state != ProcessState::Prepared {
            return Err(ProcessError::InvalidState);
        }
        if process.task.is_some() {
            return Err(ProcessError::TaskAlreadyReserved);
        }
    }

    let handle = scheduler::reserve_ring3_task(next_context_ref()).map_err(ProcessError::Scheduler)?;

    let associated = {
        let mut table = PROCESS_TABLE.lock();
        match table.get_mut(pid) {
            Some(process)
                if process.state == ProcessState::Prepared && process.task.is_none() =>
            {
                process.task = Some(handle);
                true
            }
            _ => false,
        }
    };

    if !associated {
        let _ = scheduler::cancel_ring3_reservation(handle);
        return Err(ProcessError::TaskAlreadyReserved);
    }
    Ok(handle)
}

/// Publish only after the caller has built the complete initial interrupt frame
/// on the reserved task's pinned kernel stack.
pub fn publish_process_task(pid: u32, saved_rsp: usize) -> Result<(), ProcessError> {
    let handle = {
        let mut table = PROCESS_TABLE.lock();
        let process = table.get_mut(pid).ok_or(ProcessError::NotFound)?;
        if process.state != ProcessState::Prepared {
            return Err(ProcessError::InvalidState);
        }
        let handle = process.task.ok_or(ProcessError::TaskNotReserved)?;
        process.state = ProcessState::Publishing;
        handle
    };

    if let Err(error) = scheduler::publish_ring3_task(handle, saved_rsp) {
        if let Some(process) = PROCESS_TABLE.lock().get_mut(pid) {
            if process.task == Some(handle) && process.state == ProcessState::Publishing {
                process.state = ProcessState::Prepared;
            }
        }
        return Err(ProcessError::Scheduler(error));
    }

    if let Some(process) = PROCESS_TABLE.lock().get_mut(pid) {
        if process.task == Some(handle) && process.state == ProcessState::Publishing {
            process.state = ProcessState::Runnable;
            return Ok(());
        }
    }

    Err(ProcessError::InvalidState)
}

/// Cancel a reservation only while its process remains unpublished.
pub fn cancel_process_task(pid: u32) -> Result<(), ProcessError> {
    let handle = {
        let table = PROCESS_TABLE.lock();
        let process = table.get(pid).ok_or(ProcessError::NotFound)?;
        if process.state != ProcessState::Prepared {
            return Err(ProcessError::InvalidState);
        }
        process.task.ok_or(ProcessError::TaskNotReserved)?
    };

    scheduler::cancel_ring3_reservation(handle).map_err(ProcessError::Scheduler)?;
    let mut table = PROCESS_TABLE.lock();
    let process = table.get_mut(pid).ok_or(ProcessError::NotFound)?;
    if process.state != ProcessState::Prepared || process.task != Some(handle) {
        return Err(ProcessError::InvalidState);
    }
    process.task = None;
    Ok(())
}

/// Mark a published process as the active owner on `cpu_index` before entering
/// Ring 3. The CPU-qualified handle prevents a cross-CPU activation.
pub fn activate_process(cpu_index: usize, pid: u32) -> Result<(), ProcessError> {
    if cpu_index >= scheduler::MAX_CPUS {
        return Err(ProcessError::WrongCpu);
    }
    let mut table = PROCESS_TABLE.lock();
    let process = table.get_mut(pid).ok_or(ProcessError::NotFound)?;
    if process.state != ProcessState::Runnable && process.state != ProcessState::Running {
        return Err(ProcessError::InvalidState);
    }
    let handle = process.task.ok_or(ProcessError::TaskNotReserved)?;
    if handle.cpu_index() != cpu_index {
        return Err(ProcessError::WrongCpu);
    }
    process.state = ProcessState::Running;
    CURRENT_PID[cpu_index].store(pid, Ordering::Release);
    Ok(())
}

/// Publish a single, idempotent voluntary exit for the current process.
pub fn mark_process_exited(cpu_index: usize, pid: u32, exit_code: u32) -> Result<(), ProcessError> {
    if cpu_index >= scheduler::MAX_CPUS {
        return Err(ProcessError::WrongCpu);
    }
    let mut table = PROCESS_TABLE.lock();
    let process = table.get_mut(pid).ok_or(ProcessError::NotFound)?;
    if process.state == ProcessState::Exited {
        return Err(ProcessError::AlreadyExited);
    }
    if process.state != ProcessState::Running {
        return Err(ProcessError::InvalidState);
    }
    let handle = process.task.ok_or(ProcessError::TaskNotReserved)?;
    if handle.cpu_index() != cpu_index {
        return Err(ProcessError::WrongCpu);
    }
    process.state = ProcessState::Exited;
    process.exit_code = Some(exit_code);
    if CURRENT_PID[cpu_index].load(Ordering::Acquire) == pid {
        CURRENT_PID[cpu_index].store(0, Ordering::Release);
    }
    Ok(())
}

/// Reap an exited process: close its descriptors and destroy its private space.
/// The caller must have switched off the process CR3 before calling.
pub fn reap_process(
    pid: u32,
    allocator: &mut crate::memory::PhysicalFrameAllocator,
) -> Result<(), ProcessError> {
    {
        let table = PROCESS_TABLE.lock();
        let process = table.get(pid).ok_or(ProcessError::NotFound)?;
        if process.state != ProcessState::Exited {
            return Err(ProcessError::InvalidState);
        }
        if process
            .address_space_root()
            .is_some_and(|root| cr3_root(unsafe { read_cr3() }) == root)
        {
            return Err(ProcessError::ActiveAddressSpace);
        }
    }

    let mut process = PROCESS_TABLE
        .lock()
        .remove(pid)
        .ok_or(ProcessError::NotFound)?;
    let handles = process.file_table.take_all();
    let _ = rootfs::with_root(|vfs| {
        for handle in handles.iter().flatten().copied() {
            let _ = vfs.close(handle);
        }
    });
    let space = process.address_space.take().ok_or(ProcessError::InvalidState)?;
    // SAFETY: The active CR3 was checked above and the caller owns the allocator.
    unsafe { space.destroy(allocator).map_err(ProcessError::AddressSpace)? }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReapEntry {
    pid: u32,
    cpu_index: usize,
    root: u64,
    /// Snapshot of CPUs referencing this space when the exit was published.
    target_mask: u64,
    /// Non-zero while a shootdown has been issued but not fully acknowledged.
    shootdown_generation: u64,
}

/// CPU-affine deferred-destruction queue. The owner CPU briefly takes this lock
/// to pop or requeue an entry, then releases it before any shootdown or allocator
/// work. No scheduler run-queue lock is ever held across these operations.
static REAP_QUEUE: SpinLock<[Option<ReapEntry>; MAX_PROCESSES]> = SpinLock::new([None; MAX_PROCESSES]);
static REAP_ENQUEUED: AtomicU64 = AtomicU64::new(0);
static REAP_DESTROYED: AtomicU64 = AtomicU64::new(0);
static REAP_DEFERRED: AtomicU64 = AtomicU64::new(0);
static REAP_SHOOTDOWNS: AtomicU64 = AtomicU64::new(0);
static REAP_SHOOTDOWN_ACKS: AtomicU64 = AtomicU64::new(0);

/// RAII reader pin: prevents deferred destruction while a context is inspected.
pub struct ProcessPin(u32);

impl Drop for ProcessPin {
    fn drop(&mut self) {
        unpin_process(self.0);
    }
}

/// Take a reader pin on a live process. A process already being reaped refuses
/// new pins, so a late reader can never keep a destroyed space alive.
pub fn pin_process(pid: u32) -> Option<ProcessPin> {
    let table = PROCESS_TABLE.lock();
    let process = table.get(pid)?;
    if matches!(process.state, ProcessState::Reaping | ProcessState::Reaped)
        || process.closing.load(Ordering::Acquire)
    {
        return None;
    }
    process.pin_count.fetch_add(1, Ordering::AcqRel);
    Some(ProcessPin(pid))
}

pub fn unpin_process(pid: u32) {
    let table = PROCESS_TABLE.lock();
    if let Some(process) = table.get(pid) {
        let _ = process.pin_count.fetch_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |count| count.checked_sub(1),
        );
    }
}

/// Record that `cpu_index` installed `root` as CR3. Unknown roots (kernel idle
/// space) are ignored; new activations of a closing space are refused.
pub fn mark_address_space_active(root: u64, cpu_index: usize) -> bool {
    if cpu_index >= scheduler::MAX_CPUS || root == 0 {
        return false;
    }
    let table = PROCESS_TABLE.lock();
    for process in table.get_slots().iter().flatten() {
        if process.address_space_root() == Some(root) && !process.closing.load(Ordering::Acquire) {
            process
                .active_cpu_mask
                .fetch_or(1u64 << cpu_index, Ordering::AcqRel);
            return true;
        }
    }
    false
}

/// Remove `cpu_index` from the active mask of the process owning `root`.
pub fn mark_address_space_inactive(root: u64, cpu_index: usize) -> bool {
    if cpu_index >= scheduler::MAX_CPUS || root == 0 {
        return false;
    }
    let table = PROCESS_TABLE.lock();
    for process in table.get_slots().iter().flatten() {
        if process.address_space_root() == Some(root) {
            process
                .active_cpu_mask
                .fetch_and(!(1u64 << cpu_index), Ordering::AcqRel);
            return true;
        }
    }
    false
}

pub fn process_pin_count(pid: u32) -> Option<usize> {
    let table = PROCESS_TABLE.lock();
    table.get(pid).map(|process| process.pin_count.load(Ordering::Acquire))
}

pub fn process_active_cpu_mask(pid: u32) -> Option<u64> {
    let table = PROCESS_TABLE.lock();
    table.get(pid).map(|process| process.active_cpu_mask.load(Ordering::Acquire))
}

/// Move an exited process into the deferred reaper queue. This is the only path
/// from task termination to resource reclamation; it never destroys directly.
pub fn queue_process_for_reap(pid: u32) -> Result<(), ProcessError> {
    let entry = {
        let mut table = PROCESS_TABLE.lock();
        let process = table.get_mut(pid).ok_or(ProcessError::NotFound)?;
        if process.state != ProcessState::Exited {
            return Err(ProcessError::InvalidState);
        }
        let root = process.address_space_root().ok_or(ProcessError::InvalidState)?;
        let cpu_index = process
            .task
            .map(|handle| handle.cpu_index())
            .ok_or(ProcessError::InvalidState)?;
        let target_mask = process.active_cpu_mask.load(Ordering::Acquire);
        process.closing.store(true, Ordering::Release);
        process.state = ProcessState::Reaping;
        ReapEntry { pid, cpu_index, root, target_mask, shootdown_generation: 0 }
    };
    let mut queue = REAP_QUEUE.lock();
    let slot = queue
        .iter_mut()
        .find(|slot| slot.is_none())
        .ok_or(ProcessError::TableFull)?;
    *slot = Some(entry);
    REAP_ENQUEUED.fetch_add(1, Ordering::AcqRel);
    Ok(())
}

fn requeap_entry(entry: ReapEntry) {
    let mut queue = REAP_QUEUE.lock();
    if let Some(slot) = queue.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(entry);
    }
}

/// Destroy a process that is already in `Reaping` and has no pins or active CPUs.
fn destroy_reaped_process(
    pid: u32,
    allocator: &mut crate::memory::PhysicalFrameAllocator,
) -> Result<(), ProcessError> {
    let mut process = PROCESS_TABLE.lock().remove(pid).ok_or(ProcessError::NotFound)?;
    if process.state != ProcessState::Reaping {
        return Err(ProcessError::InvalidState);
    }
    process.state = ProcessState::Reaped;
    let handles = process.file_table.take_all();
    let _ = rootfs::with_root(|vfs| {
        for handle in handles.iter().flatten().copied() {
            let _ = vfs.close(handle);
        }
    });
    let space = process.address_space.take().ok_or(ProcessError::InvalidState)?;
    // SAFETY: pins and the active CPU mask were checked before destruction, and
    // the shootdown acknowledged every CPU in the snapshot mask.
    unsafe { space.destroy(allocator).map_err(ProcessError::AddressSpace)? }
    Ok(())
}

/// Drain this CPU's reaper entries. Entries with outstanding pins or active CPUs
/// are requeued without taking any scheduler lock or waiting for an ACK inside
/// the queue lock.
pub fn drain_reaper(
    cpu_index: usize,
    allocator: &mut crate::memory::PhysicalFrameAllocator,
) -> Result<usize, ProcessError> {
    if cpu_index >= scheduler::MAX_CPUS {
        return Err(ProcessError::WrongCpu);
    }
    let mut taken = [None; MAX_PROCESSES];
    {
        let mut queue = REAP_QUEUE.lock();
        let mut used = 0;
        for slot in queue.iter_mut() {
            if let Some(entry) = *slot {
                if entry.cpu_index == cpu_index {
                    taken[used] = Some(entry);
                    used += 1;
                    *slot = None;
                }
            }
        }
    }

    let mut destroyed = 0;
    for entry in taken.iter().flatten().copied() {
        let mut entry = entry;
        let ready = {
            let table = PROCESS_TABLE.lock();
            let Some(process) = table.get(entry.pid) else { continue };
            process.pin_count.load(Ordering::Acquire) == 0
                && process.active_cpu_mask.load(Ordering::Acquire) == 0
        };
        // A pending shootdown is checked without spin-waiting. The entry returns
        // to the queue and destruction resumes on a later drain once every
        // remote CPU has acknowledged the generation.
        let pending_shootdown = if entry.shootdown_generation != 0 {
            let status = crate::address_space::tlb_shootdown_status(
                entry.target_mask,
                entry.shootdown_generation,
            );
            status.acks < status.targets
        } else {
            false
        };
        if !ready || pending_shootdown {
            requeap_entry(entry);
            REAP_DEFERRED.fetch_add(1, Ordering::AcqRel);
            continue;
        }
        if entry.shootdown_generation == 0 && entry.target_mask != 0 {
            let stats = crate::address_space::tlb_shootdown(entry.root, entry.target_mask);
            REAP_SHOOTDOWNS.fetch_add(1, Ordering::AcqRel);
            REAP_SHOOTDOWN_ACKS.fetch_add(stats.acks as u64, Ordering::AcqRel);
            if stats.acks < stats.targets {
                entry.shootdown_generation = stats.generation;
                requeap_entry(entry);
                REAP_DEFERRED.fetch_add(1, Ordering::AcqRel);
                continue;
            }
        }
        destroy_reaped_process(entry.pid, allocator)?;
        destroyed += 1;
        REAP_DESTROYED.fetch_add(1, Ordering::AcqRel);
    }
    Ok(destroyed)
}

pub fn reaper_pending_count() -> usize {
    REAP_QUEUE.lock().iter().flatten().count()
}

pub fn reaper_enqueued_count() -> u64 {
    REAP_ENQUEUED.load(Ordering::Acquire)
}

pub fn reaper_destroyed_count() -> u64 {
    REAP_DESTROYED.load(Ordering::Acquire)
}

pub fn reaper_deferred_count() -> u64 {
    REAP_DEFERRED.load(Ordering::Acquire)
}

pub fn reaper_shootdown_count() -> u64 {
    REAP_SHOOTDOWNS.load(Ordering::Acquire)
}

pub fn reaper_shootdown_ack_count() -> u64 {
    REAP_SHOOTDOWN_ACKS.load(Ordering::Acquire)
}

pub fn current_process(cpu_index: usize) -> Option<u32> {
    CURRENT_PID
        .get(cpu_index)
        .map(|pid| pid.load(Ordering::Acquire))
        .filter(|pid| *pid != 0)
}

pub fn process_state(pid: u32) -> Option<ProcessState> {
    PROCESS_TABLE.lock().get(pid).map(|process| process.state)
}

pub fn process_task_handle(pid: u32) -> Option<Ring3TaskHandle> {
    PROCESS_TABLE.lock().get(pid).and_then(|process| process.task)
}

/// Reverse lookup: which process owns a CPU-qualified scheduler handle?
pub fn process_for_task_handle(handle: Ring3TaskHandle) -> Option<u32> {
    PROCESS_TABLE
        .lock()
        .get_slots()
        .iter()
        .flatten()
        .find(|process| process.task == Some(handle))
        .map(|process| process.pid)
}

/// Reverse lookup by the scheduler's local task id.
pub fn process_for_task_id(task_id: TaskId) -> Option<u32> {
    PROCESS_TABLE
        .lock()
        .get_slots()
        .iter()
        .flatten()
        .find(|process| process.task.is_some_and(|handle| handle.task_id() == task_id))
        .map(|process| process.pid)
}

impl ProcessTable {
    fn get_slots(&self) -> &[Option<Process>; MAX_PROCESSES] {
        &self.slots
    }
}

#[cfg(test)]
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub fn reset_reaper_for_test() {
    let mut queue = REAP_QUEUE.lock();
    for slot in queue.iter_mut() {
        *slot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn noop_task() {}

    fn prepare_scheduler() {
        scheduler::initialize_for_process_test();
        NEXT_CONTEXT_ID.store(1, Ordering::Release);
        PROCESS_TABLE.lock().reset_for_test();
        reset_reaper_for_test();
    }

    #[test]
    fn process_to_task_binding_is_bidirectional_and_exclusive() {
        let _guard = TEST_LOCK.lock().unwrap();
        prepare_scheduler();

        let (pid, handle) = create_process_with_task(
            AddressSpace::for_test(0x1000),
            0x2000,
            0x3000,
            ProcessFileTable::new(),
        )
        .unwrap();

        assert_eq!(process_task_handle(pid), Some(handle));
        assert_eq!(process_for_task_handle(handle), Some(pid));
        assert_eq!(process_for_task_id(handle.task_id()), Some(pid));
        assert_eq!(process_state(pid), Some(ProcessState::Prepared));

        let saved_rsp = scheduler::prepare_ring3_task(handle, noop_task).unwrap();
        publish_process_task(pid, saved_rsp).unwrap();
        assert_eq!(process_state(pid), Some(ProcessState::Runnable));
        activate_process(0, pid).unwrap();
        assert_eq!(current_process(0), Some(pid));
        mark_process_exited(0, pid, 42).unwrap();
        assert_eq!(process_state(pid), Some(ProcessState::Exited));
        assert_eq!(process_task_handle(pid), Some(handle));
        assert_eq!(process_for_task_handle(handle), Some(pid));
        assert_eq!(current_process(0), None);
        assert_eq!(mark_process_exited(0, pid, 43), Err(ProcessError::AlreadyExited));
        scheduler::reset_for_process_test();
    }

    #[test]
    fn failed_publication_rolls_back_and_a_stale_handle_cannot_be_reused() {
        let _guard = TEST_LOCK.lock().unwrap();
        prepare_scheduler();

        let (pid, handle) = create_process_with_task(
            AddressSpace::for_test(0x4000),
            0x5000,
            0x6000,
            ProcessFileTable::new(),
        )
        .unwrap();

        assert_eq!(
            publish_process_task(pid, 1),
            Err(ProcessError::Scheduler(SchedulerError::InvalidSavedStack))
        );
        assert_eq!(process_state(pid), Some(ProcessState::Prepared));
        assert_eq!(process_task_handle(pid), Some(handle));

        // The failed publication did not consume the reservation: a valid retry works.
        let saved_rsp = scheduler::prepare_ring3_task(handle, noop_task).unwrap();
        publish_process_task(pid, saved_rsp).unwrap();
        assert_eq!(process_state(pid), Some(ProcessState::Runnable));
        scheduler::reset_for_process_test();
    }

    #[test]
    fn pins_and_active_cpu_mask_defer_reaper_destruction() {
        let _guard = TEST_LOCK.lock().unwrap();
        prepare_scheduler();
        let deferred_before = reaper_deferred_count();

        let (pid, handle) = create_process_with_task(
            AddressSpace::for_test(0x7000),
            0x8000,
            0x9000,
            ProcessFileTable::new(),
        )
        .unwrap();
        let saved_rsp = scheduler::prepare_ring3_task(handle, noop_task).unwrap();
        publish_process_task(pid, saved_rsp).unwrap();
        activate_process(0, pid).unwrap();

        assert!(mark_address_space_active(0x7000, 0));
        assert_eq!(process_active_cpu_mask(pid), Some(1));
        let pin = pin_process(pid).unwrap();
        assert_eq!(process_pin_count(pid), Some(1));

        mark_process_exited(0, pid, 7).unwrap();
        queue_process_for_reap(pid).unwrap();
        assert_eq!(process_state(pid), Some(ProcessState::Reaping));
        assert!(pin_process(pid).is_none());
        assert_eq!(reaper_pending_count(), 1);

        // No allocator work is done while a pin or an active CPU still exists.
        let mut allocator = crate::memory::PhysicalFrameAllocator::empty();
        assert_eq!(drain_reaper(0, &mut allocator), Ok(0));
        assert_eq!(reaper_pending_count(), 1);
        assert!(reaper_deferred_count() > deferred_before);

        drop(pin);
        assert!(mark_address_space_inactive(0x7000, 0));
        assert_eq!(process_pin_count(pid), Some(0));
        assert_eq!(process_active_cpu_mask(pid), Some(0));

        // The reaper queue is drained by CPU affinity; a different CPU must not
        // consume this owner's entry.
        assert_eq!(drain_reaper(1, &mut allocator), Ok(0));
        assert_eq!(reaper_pending_count(), 1);
        scheduler::reset_for_process_test();
    }

    #[test]
    fn reaping_requires_a_terminal_process() {
        let _guard = TEST_LOCK.lock().unwrap();
        prepare_scheduler();

        let (pid, _handle) = create_process_with_task(
            AddressSpace::for_test(0x7000),
            0x8000,
            0x9000,
            ProcessFileTable::new(),
        )
        .unwrap();

        // No allocator is needed on the invalid-state path.
        let mut allocator = crate::memory::PhysicalFrameAllocator::empty();
        assert_eq!(
            reap_process(pid, &mut allocator),
            Err(ProcessError::InvalidState)
        );
        assert_eq!(process_state(pid), Some(ProcessState::Prepared));
        scheduler::reset_for_process_test();
    }
}

#[cfg(test)]
impl ProcessTable {
    fn reset_for_test(&mut self) {
        for slot in self.slots.iter_mut() {
            *slot = None;
        }
        self.next_pid = 1;
    }
}
