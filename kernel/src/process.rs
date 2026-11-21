//! SMP-safe ownership of Ring 3 process metadata and scheduler reservations.
//!
//! Process metadata is protected by one short-held lock. Scheduler operations are
//! always performed after releasing that lock; this avoids lock-order inversion
//! with per-CPU run-queue locks. Address spaces remain owned by their Process.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{
    address_space::AddressSpace,
    file_descriptors::ProcessFileTable,
    scheduler::{self, Ring3TaskContextRef, Ring3TaskHandle, SchedulerError},
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
}

pub struct Process {
    pid: u32,
    state: ProcessState,
    address_space: AddressSpace,
    entry: u64,
    user_stack: u64,
    task: Option<Ring3TaskHandle>,
    file_table: ProcessFileTable,
    exit_code: Option<u32>,
}

impl Process {
    fn new(
        pid: u32,
        address_space: AddressSpace,
        entry: u64,
        user_stack: u64,
        file_table: ProcessFileTable,
    ) -> Self {
        Self {
            pid,
            state: ProcessState::Prepared,
            address_space,
            entry,
            user_stack,
            task: None,
            file_table,
            exit_code: None,
        }
    }

    pub const fn pid(&self) -> u32 { self.pid }
    pub const fn state(&self) -> ProcessState { self.state }
    pub const fn entry(&self) -> u64 { self.entry }
    pub const fn user_stack(&self) -> u64 { self.user_stack }
    pub const fn address_space_root(&self) -> u64 { self.address_space.root() }
    pub const fn task_handle(&self) -> Option<Ring3TaskHandle> { self.task }
    pub const fn exit_code(&self) -> Option<u32> { self.exit_code }

    pub fn file_table(&self) -> &ProcessFileTable { &self.file_table }
    pub fn file_table_mut(&mut self) -> &mut ProcessFileTable { &mut self.file_table }
}

struct ProcessTable {
    slots: [Option<Process>; MAX_PROCESSES],
    next_pid: u32,
}

impl ProcessTable {
    const fn new() -> Self {
        Self { slots: [const { None }; MAX_PROCESSES], next_pid: 1 }
    }

    fn insert(
        &mut self,
        address_space: AddressSpace,
        entry: u64,
        user_stack: u64,
        file_table: ProcessFileTable,
    ) -> Result<u32, ProcessError> {
        let slot = self.slots.iter().position(Option::is_none).ok_or(ProcessError::TableFull)?;
        let pid = self.next_pid;
        self.next_pid = self.next_pid.checked_add(1).ok_or(ProcessError::PidExhausted)?;
        if pid == 0 {
            return Err(ProcessError::PidExhausted);
        }
        self.slots[slot] = Some(Process::new(pid, address_space, entry, user_stack, file_table));
        Ok(pid)
    }

    fn get(&self, pid: u32) -> Option<&Process> {
        self.slots.iter().flatten().find(|process| process.pid == pid)
    }

    fn get_mut(&mut self, pid: u32) -> Option<&mut Process> {
        self.slots.iter_mut().flatten().find(|process| process.pid == pid)
    }
}

static PROCESS_TABLE: SpinLock<ProcessTable> = SpinLock::new(ProcessTable::new());
static CURRENT_PID: [AtomicU32; scheduler::MAX_CPUS] =
    [const { AtomicU32::new(0) }; scheduler::MAX_CPUS];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessError {
    TableFull,
    PidExhausted,
    NotFound,
    InvalidState,
    TaskAlreadyReserved,
    TaskNotReserved,
    WrongCpu,
    Scheduler(SchedulerError),
}

/// Insert a fully constructed address space as a non-runnable process.
pub fn create_prepared_process(
    address_space: AddressSpace,
    entry: u64,
    user_stack: u64,
    file_table: ProcessFileTable,
) -> Result<u32, ProcessError> {
    PROCESS_TABLE.lock().insert(address_space, entry, user_stack, file_table)
}

/// Reserve a scheduler slot and atomically associate its CPU-qualified handle with a PID.
///
/// The process-table lock is never held while taking a scheduler run-queue lock.
/// If association cannot be committed, the unpublished scheduler reservation is rolled back.
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

    let handle = scheduler::reserve_ring3_task(Ring3TaskContextRef::new(pid as usize))
        .map_err(ProcessError::Scheduler)?;

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

    // The table entry is not expected to disappear; do not silently claim success
    // if a future lifecycle API violates that invariant.
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

/// Set the current PID in the calling CPU's slot; cross-CPU handles are rejected.
pub fn set_current_process(cpu_index: usize, pid: u32) -> Result<(), ProcessError> {
    if cpu_index >= scheduler::MAX_CPUS {
        return Err(ProcessError::WrongCpu);
    }
    if pid != 0 {
        let table = PROCESS_TABLE.lock();
        let process = table.get(pid).ok_or(ProcessError::NotFound)?;
        let handle = process.task.ok_or(ProcessError::TaskNotReserved)?;
        if handle.cpu_index() != cpu_index || process.state != ProcessState::Runnable {
            return Err(ProcessError::WrongCpu);
        }
    }
    CURRENT_PID[cpu_index].store(pid, Ordering::Release);
    Ok(())
}

pub fn current_process(cpu_index: usize) -> Option<u32> {
    CURRENT_PID.get(cpu_index).map(|pid| pid.load(Ordering::Acquire)).filter(|pid| *pid != 0)
}

pub fn process_state(pid: u32) -> Option<ProcessState> {
    PROCESS_TABLE.lock().get(pid).map(|process| process.state)
}

pub fn process_task_handle(pid: u32) -> Option<Ring3TaskHandle> {
    PROCESS_TABLE.lock().get(pid).and_then(|process| process.task)
}
