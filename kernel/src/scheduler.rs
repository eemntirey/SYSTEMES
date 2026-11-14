//! Per-CPU kernel task schedulers for x86_64.
//!
//! Each initialized CPU owns a bounded task queue and schedules it through
//! cooperative yield or Local APIC timer preemption. Runnable contexts can be
//! stolen from another CPU under two queue locks acquired in ascending CPU order.
//! Migration transfers ownership of a pinned stack/FXSAVE resource pair and the
//! saved context, then retires the donor slot; RUNNING tasks are never stealable.

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering};

pub const MAX_TASKS: usize = 8;
pub const MAX_CPUS: usize = 64;
const STACK_SIZE: usize = 16 * 1024;
const FPU_STATE_SIZE: usize = 512;
const YIELD_VECTOR: usize = 0x41;
const KERNEL_CODE_SELECTOR: usize = 0x08;
const KERNEL_DATA_SELECTOR: usize = 0x10;
const INITIAL_RFLAGS: usize = 0x202;
const SAVED_REGISTER_COUNT: usize = 15;
const INTERRUPT_FRAME_WORDS: usize = SAVED_REGISTER_COUNT + 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TaskState {
    Empty = 0,
    Runnable = 1,
    Blocked = 2,
    Exited = 3,
    Running = 4,
    Migrating = 5,
}

impl TaskState {
    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Runnable,
            2 => Self::Blocked,
            3 => Self::Exited,
            4 => Self::Running,
            5 => Self::Migrating,
            _ => Self::Empty,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskId(pub usize);

/// Opaque stable identifier for the owner-managed Ring 3 context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ring3TaskContextRef(usize);

impl Ring3TaskContextRef {
    pub const fn new(id: usize) -> Self { Self(id) }
}

/// Fixed-size diagnostic captured once when a Ring 3 task exits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskExitReason {
    VoluntaryExit { code: i32 },
    PageFault { cr2: usize, error_code: usize, rip: usize },
    GeneralProtection { error_code: usize, rip: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    NotInitialized,
    InvalidTask,
    BootTask,
    CurrentTask,
    ContextAlreadyAttached,
    InvalidTaskState,
    AlreadyExited,
}

pub type TaskEntry = extern "C" fn();

#[repr(C, align(16))]
struct FpuState([u8; FPU_STATE_SIZE]);

#[repr(align(16))]
struct TaskStack([u8; STACK_SIZE]);

struct Task {
    /// Stable resource-pool slot; unlike the queue slot, this never changes during a task's life.
    resource_id: AtomicUsize,
    saved_rsp: AtomicUsize,
    state: AtomicU8,
    switches: AtomicUsize,
    entry: AtomicUsize,
    wake_at: AtomicU64,
    migrations: AtomicUsize,
    ring3_context_id: AtomicUsize,
    cpu_affine: AtomicBool,
    /// Written under run_queue_lock before Exited is published; immutable afterwards.
    exit_reason: Option<TaskExitReason>,
}

impl Task {
    const fn new() -> Self {
        Self {
            resource_id: AtomicUsize::new(usize::MAX),
            saved_rsp: AtomicUsize::new(0),
            state: AtomicU8::new(TaskState::Empty as u8),
            switches: AtomicUsize::new(0),
            entry: AtomicUsize::new(0),
            wake_at: AtomicU64::new(0),
            migrations: AtomicUsize::new(0),
            ring3_context_id: AtomicUsize::new(usize::MAX),
            cpu_affine: AtomicBool::new(false),
            exit_reason: None,
        }
    }

    /// Caller holds this CPU's run_queue_lock. The first caller stores the reason
    /// before publishing Exited; later callers cannot overwrite it.
    fn try_mark_exited(&mut self, reason: TaskExitReason) -> bool {
        let state = self.state.load(Ordering::Acquire);
        if state != TaskState::Running as u8
            && state != TaskState::Runnable as u8
            && state != TaskState::Blocked as u8
        {
            return false;
        }
        self.exit_reason = Some(reason);
        self.state.store(TaskState::Exited as u8, Ordering::Release);
        true
    }
}

/// Stack and FPU resources stay at fixed addresses while their task descriptors migrate.
struct TaskResources {
    stack: TaskStack,
    fpu: FpuState,
}

impl TaskResources {
    const fn new() -> Self {
        Self { stack: TaskStack([0; STACK_SIZE]), fpu: FpuState([0; FPU_STATE_SIZE]) }
    }
}

/// Per-CPU scheduler state.
struct CpuScheduler {
    tasks: [Task; MAX_TASKS],
    task_count: AtomicUsize,
    current_task: AtomicUsize,
    initialized: AtomicBool,
    preemptive_enabled: AtomicBool,
    preemption_count: AtomicUsize,
    fpu_contexts_ready: AtomicBool,
    /// Serializes local scheduling with cross-CPU queue transfers.
    run_queue_lock: AtomicBool,
}

/// Array of per-CPU schedulers, indexed by CPU slot (APIC ID).
static mut CPU_SCHEDULERS: [CpuScheduler; MAX_CPUS] = [const {
    CpuScheduler {
        tasks: [const { Task::new() }; MAX_TASKS],
        task_count: AtomicUsize::new(1),
        current_task: AtomicUsize::new(0),
        initialized: AtomicBool::new(false),
        preemptive_enabled: AtomicBool::new(false),
        preemption_count: AtomicUsize::new(0),
        fpu_contexts_ready: AtomicBool::new(false),
        run_queue_lock: AtomicBool::new(false),
    }
}; MAX_CPUS];

/// Global fixed-address backing store: task descriptors may move between CPU queues,
/// but their kernel stacks and FXSAVE buffers never move or get copied.
static mut TASK_RESOURCES: [TaskResources; MAX_CPUS * MAX_TASKS] =
    [const { TaskResources::new() }; MAX_CPUS * MAX_TASKS];

fn task_resource_ptr(resource_id: usize) -> *mut TaskResources {
    debug_assert!(resource_id < MAX_CPUS * MAX_TASKS);
    // SAFETY: resource_id is assigned once at task initialization and transferred atomically.
    unsafe { core::ptr::addr_of_mut!(TASK_RESOURCES).cast::<TaskResources>().add(resource_id) }
}

fn cpu_scheduler(cpu_index: usize) -> *mut CpuScheduler {
    debug_assert!(cpu_index < MAX_CPUS);
    // SAFETY: CPU_SCHEDULERS is a static array; caller validates cpu_index.
    unsafe { core::ptr::addr_of_mut!(CPU_SCHEDULERS).cast::<CpuScheduler>().add(cpu_index) }
}

static MIGRATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static MIGRATED_TASK_EXECUTIONS: AtomicUsize = AtomicUsize::new(0);

fn lock_run_queue(cpu_index: usize) {
    let sched = unsafe { cpu_scheduler(cpu_index) };
    while unsafe { (*sched).run_queue_lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed) }.is_err() {
        core::hint::spin_loop();
    }
}

fn unlock_run_queue(cpu_index: usize) {
    let sched = unsafe { cpu_scheduler(cpu_index) };
    unsafe { (*sched).run_queue_lock.store(false, Ordering::Release); }
}

/// Total successful cross-CPU task transfers since boot.
pub fn migration_count() -> usize {
    MIGRATION_COUNT.load(Ordering::Acquire)
}

/// Return how many times the currently running task has crossed a CPU boundary.
pub fn current_task_migration_count() -> usize {
    let cpu = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu) };
    let current = unsafe { (*sched).current_task.load(Ordering::Acquire) };
    if current >= unsafe { (*sched).task_count.load(Ordering::Acquire) } { return 0; }
    unsafe { (*task_ptr(cpu, current)).migrations.load(Ordering::Acquire) }
}

/// Record execution from the receiving CPU after a migrated task resumes.
pub fn record_migrated_task_execution() -> bool {
    if current_task_migration_count() == 0 { return false; }
    MIGRATED_TASK_EXECUTIONS.fetch_add(1, Ordering::AcqRel);
    true
}

pub fn migrated_task_execution_count() -> usize {
    MIGRATED_TASK_EXECUTIONS.load(Ordering::Acquire)
}

/// Move one READY task from a donor CPU into an idle receiver's private task slot.
/// Both run-queue locks are always acquired in ascending CPU-index order.
fn steal_task(receiver: usize) -> bool {
    if receiver >= MAX_CPUS { return false; }
    let receiver_sched = unsafe { cpu_scheduler(receiver) };
    if !unsafe { (*receiver_sched).initialized.load(Ordering::Acquire) } { return false; }

    for donor in 0..MAX_CPUS {
        if donor == receiver { continue; }
        let donor_sched = unsafe { cpu_scheduler(donor) };
        if !unsafe { (*donor_sched).initialized.load(Ordering::Acquire) } { continue; }

        let first = core::cmp::min(receiver, donor);
        let second = core::cmp::max(receiver, donor);
        lock_run_queue(first);
        lock_run_queue(second);
        // A CPU may have gone offline/reset while this thief waited for the locks.
        if !unsafe { (*receiver_sched).initialized.load(Ordering::Acquire) }
            || !unsafe { (*donor_sched).initialized.load(Ordering::Acquire) }
        {
            unlock_run_queue(second);
            unlock_run_queue(first);
            continue;
        }

        let donor_count = unsafe { (*donor_sched).task_count.load(Ordering::Acquire) };
        let donor_current = unsafe { (*donor_sched).current_task.load(Ordering::Acquire) };
        let receiver_count = unsafe { (*receiver_sched).task_count.load(Ordering::Acquire) };
        let mut destination = None;
        for index in 1..receiver_count {
            if unsafe { (*task_ptr(receiver, index)).state.load(Ordering::Acquire) } == TaskState::Empty as u8 {
                destination = Some(index);
                break;
            }
        }
        if destination.is_none() && receiver_count < MAX_TASKS {
            destination = Some(receiver_count);
        }

        let mut transferred = false;
        if let Some(dst_index) = destination {
            for src_index in 1..donor_count {
                if src_index == donor_current { continue; }
                let src = task_ptr(donor, src_index);
                if unsafe { (*src).state.load(Ordering::Acquire) } != TaskState::Runnable as u8 { continue; }
                // Ring 3 contexts are CPU-affine until CR3/TSS/GS state is migratable.
                if unsafe { (*src).cpu_affine.load(Ordering::Acquire) } { continue; }
                let resource_id = unsafe { (*src).resource_id.load(Ordering::Acquire) };
                let src_rsp = unsafe { (*src).saved_rsp.load(Ordering::Acquire) };
                if resource_id >= MAX_CPUS * MAX_TASKS || src_rsp == 0 { continue; }
                let src_resource = unsafe { task_resource_ptr(resource_id) };
                let src_stack_base = unsafe { core::ptr::addr_of!((*src_resource).stack.0) as usize };
                if src_rsp < src_stack_base || src_rsp >= src_stack_base + STACK_SIZE { continue; }
                if unsafe { (*src).state.compare_exchange(TaskState::Runnable as u8, TaskState::Migrating as u8, Ordering::AcqRel, Ordering::Acquire) }.is_err() { continue; }

                let dst = task_ptr(receiver, dst_index);
                let dst_resource_id = unsafe { (*dst).resource_id.load(Ordering::Acquire) };
                if dst_resource_id >= MAX_CPUS * MAX_TASKS || dst_resource_id == resource_id {
                    unsafe { (*src).state.store(TaskState::Runnable as u8, Ordering::Release); }
                    continue;
                }
                // The resource pool is pinned in static storage. Transfer ownership
                // of the existing stack/FPU pair; never copy a live stack or relocate
                // saved frame pointers. The empty source slot receives the destination's
                // unused resource, preserving one resource per queue slot.
                unsafe {
                    (*dst).resource_id.store(resource_id, Ordering::Relaxed);
                    (*src).resource_id.store(dst_resource_id, Ordering::Relaxed);
                    (*dst).saved_rsp.store(src_rsp, Ordering::Relaxed);
                    (*dst).switches.store((*src).switches.load(Ordering::Relaxed), Ordering::Relaxed);
                    (*dst).entry.store((*src).entry.load(Ordering::Relaxed), Ordering::Relaxed);
                    (*dst).wake_at.store((*src).wake_at.load(Ordering::Relaxed), Ordering::Relaxed);
                    (*dst).migrations.store((*src).migrations.load(Ordering::Relaxed).saturating_add(1), Ordering::Relaxed);
                    (*dst).state.store(TaskState::Runnable as u8, Ordering::Release);
                    (*src).saved_rsp.store(0, Ordering::Relaxed);
                    (*src).entry.store(0, Ordering::Relaxed);
                    (*src).wake_at.store(0, Ordering::Relaxed);
                    (*src).state.store(TaskState::Empty as u8, Ordering::Release);
                    if dst_index >= receiver_count {
                        (*receiver_sched).task_count.store(dst_index + 1, Ordering::Release);
                    }
                }
                MIGRATION_COUNT.fetch_add(1, Ordering::AcqRel);
                transferred = true;
                break;
            }
        }

        unlock_run_queue(second);
        unlock_run_queue(first);
        if transferred { return true; }
    }
    false
}

fn task_ptr(cpu_index: usize, index: usize) -> *mut Task {
    debug_assert!(cpu_index < MAX_CPUS && index < MAX_TASKS);
    let sched = cpu_scheduler(cpu_index);
    // SAFETY: Caller validates indices.
    unsafe { core::ptr::addr_of_mut!((*sched).tasks).cast::<Task>().add(index) }
}

/// Get the current CPU's scheduler index (APIC ID).
fn current_cpu_index() -> usize {
    crate::apic::current_apic_id().map(|id| id as usize).unwrap_or(0)
}

/// Configure architectural x87/SSE state management and initialize every task image.
fn initialize_fpu_contexts(cpu_index: usize) -> Result<(), &'static str> {
    // x86_64 requires SSE2; FXSAVE is the baseline state format used by this kernel.
    let features = core::arch::x86_64::__cpuid(1);
    if features.edx & (1 << 24) == 0 || features.edx & (1 << 25) == 0 {
        return Err("CPU does not support FXSAVE and SSE");
    }

    let cr0: u64;
    let cr4: u64;
    // SAFETY: This is ring-0 boot code; configure x87/SSE and disable lazy TS switching.
    unsafe {
        core::arch::asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        let next_cr0 = (cr0 | (1 << 1) | (1 << 5)) & !((1 << 2) | (1 << 3));
        let next_cr4 = cr4 | (1 << 9) | (1 << 10);
        core::arch::asm!("mov cr0, {}", in(reg) next_cr0, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mov cr4, {}", in(reg) next_cr4, options(nomem, nostack, preserves_flags));
    }

    // Preserve the boot task's incoming x87/SSE state before creating clean task images.
    let boot_task = task_ptr(cpu_index, 0);
    let boot_resource = unsafe { (*boot_task).resource_id.load(Ordering::Acquire) };
    let boot_ptr = unsafe { core::ptr::addr_of_mut!((*task_resource_ptr(boot_resource)).fpu.0).cast::<u8>() };
    let mxcsr: u32 = 0x1F80;
    // SAFETY: FPU state buffers are 16-byte aligned and at least 512 bytes long.
    unsafe {
        core::arch::asm!("fxsave64 [{}]", in(reg) boot_ptr, options(nostack));
        core::arch::asm!("fninit", "ldmxcsr [{}]", in(reg) &mxcsr, options(nostack));
        for index in 1..MAX_TASKS {
            let task = task_ptr(cpu_index, index);
            let resource_id = (*task).resource_id.load(Ordering::Acquire);
            let state = core::ptr::addr_of_mut!((*task_resource_ptr(resource_id)).fpu.0).cast::<u8>();
            core::arch::asm!("fxsave64 [{}]", in(reg) state, options(nostack));
        }
        core::arch::asm!("fxrstor64 [{}]", in(reg) boot_ptr, options(nostack));
    }
    let sched = unsafe { cpu_scheduler(cpu_index) };
    unsafe { (*sched).fpu_contexts_ready.store(true, Ordering::Release) };
    Ok(())
}

/// Save the interrupted task's complete legacy x87/MMX/SSE image before Rust IRQ code runs.
#[unsafe(no_mangle)]
pub extern "C" fn systemes_fpu_save_current() {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if !unsafe { (*sched).fpu_contexts_ready.load(Ordering::Acquire) } {
        return;
    }
    let current = unsafe { (*sched).current_task.load(Ordering::Acquire) };
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    if current >= count {
        return;
    }
    // SAFETY: The selected task owns a 16-byte-aligned 512-byte FXSAVE image.
    unsafe {
        let task = task_ptr(cpu_index, current);
        let resource_id = (*task).resource_id.load(Ordering::Acquire);
        if resource_id >= MAX_CPUS * MAX_TASKS { return; }
        let state = core::ptr::addr_of_mut!((*task_resource_ptr(resource_id)).fpu.0).cast::<u8>();
        core::arch::asm!("fxsave64 [{}]", in(reg) state, options(nostack));
    }
}

/// Restore the selected task's FPU image after Rust IRQ dispatch and before IRETQ.
#[unsafe(no_mangle)]
pub extern "C" fn systemes_fpu_restore_current() {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if !unsafe { (*sched).fpu_contexts_ready.load(Ordering::Acquire) } {
        return;
    }
    let current = unsafe { (*sched).current_task.load(Ordering::Acquire) };
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    if current >= count {
        return;
    }
    // SAFETY: The selected task owns a previously initialized 16-byte-aligned FXSAVE image.
    unsafe {
        let task = task_ptr(cpu_index, current);
        let resource_id = (*task).resource_id.load(Ordering::Acquire);
        if resource_id >= MAX_CPUS * MAX_TASKS { return; }
        let state = core::ptr::addr_of!((*task_resource_ptr(resource_id)).fpu.0).cast::<u8>();
        core::arch::asm!("fxrstor64 [{}]", in(reg) state, options(nostack));
    }
}

fn build_initial_context(_cpu_index: usize, task: *mut Task) -> usize {
    // SAFETY: The task is exclusively initialized before it becomes runnable.
    unsafe {
        let resource_id = (*task).resource_id.load(Ordering::Acquire);
        let stack = core::ptr::addr_of_mut!((*task_resource_ptr(resource_id)).stack.0).cast::<u8>();
        let top = stack.add(STACK_SIZE) as usize;
        let fake_return = (top - 8) as *mut usize;
        fake_return.write(task_returned as *const () as usize);
        let frame = (top - 8 - INTERRUPT_FRAME_WORDS * core::mem::size_of::<usize>()) as *mut usize;
        for index in 0..SAVED_REGISTER_COUNT {
            frame.add(index).write(0);
        }
        frame.add(15).write(YIELD_VECTOR);
        frame.add(16).write(0);
        frame.add(17).write(task_bootstrap as *const () as usize);
        frame.add(18).write(KERNEL_CODE_SELECTOR);
        frame.add(19).write(INITIAL_RFLAGS);
        // Long-mode IRETQ restores the synthetic RSP and SS words after RFLAGS.
        frame.add(20).write(top - 8);
        frame.add(21).write(KERNEL_DATA_SELECTOR);
        frame as usize
    }
}

extern "C" fn task_returned() -> ! {
    loop {
        // SAFETY: A task trampoline must not return to an arbitrary stack address.
        unsafe { core::arch::asm!("cli", "hlt", options(nomem, nostack)); }
    }
}

extern "C" fn task_bootstrap() -> ! {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    let current = unsafe { (*sched).current_task.load(Ordering::Acquire) };
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    if current >= count {
        task_returned();
    }
    // SAFETY: The current task index is bounded and its entry was stored before it became runnable.
    let entry_address = unsafe { (*task_ptr(cpu_index, current)).entry.load(Ordering::Acquire) };
    if entry_address == 0 {
        task_returned();
    }
    // SAFETY: initialize stores a valid extern "C" TaskEntry function pointer as usize.
    let entry: TaskEntry = unsafe { core::mem::transmute(entry_address) };
    entry();
    // SAFETY: The current task is the only writer of its exit state on this CPU.
    unsafe { (*task_ptr(cpu_index, current)).state.store(TaskState::Exited as u8, Ordering::Release); }
    loop {
        yield_now();
    }
}

/// Initialize the boot task and up to seven cooperative kernel tasks on the given CPU.
/// APs pass an empty entry list and later receive runnable tasks through work-stealing.
pub fn initialize_cpu(cpu_index: usize, entries: &[TaskEntry]) -> Result<(), &'static str> {
    if cpu_index >= MAX_CPUS {
        return Err("invalid CPU index");
    }
    // Initialization touches FPU state and queue metadata; prevent the local timer
    // from observing a partially initialized scheduler on the BSP or an AP.
    let interrupt_flags = irq_save_disable();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if unsafe { (*sched).initialized.load(Ordering::Acquire) } {
        irq_restore(interrupt_flags);
        return Err("scheduler already initialized on this CPU");
    }
    if entries.len() >= MAX_TASKS {
        irq_restore(interrupt_flags);
        return Err("too many kernel tasks");
    }
    // SAFETY: This CPU owns its scheduler slots until initialized; resource IDs
    // are fixed by the CPU/task slot and remain valid even after the descriptor migrates.
    unsafe {
        for index in 0..MAX_TASKS {
            let task = task_ptr(cpu_index, index);
            let resource_id = (*task).resource_id.load(Ordering::Acquire);
            if resource_id == usize::MAX {
                (*task).resource_id.store(cpu_index * MAX_TASKS + index, Ordering::Relaxed);
            } else if resource_id >= MAX_CPUS * MAX_TASKS {
                irq_restore(interrupt_flags);
                return Err("invalid task resource ownership");
            }
            (*task).saved_rsp.store(0, Ordering::Relaxed);
            (*task).switches.store(0, Ordering::Relaxed);
            (*task).entry.store(0, Ordering::Relaxed);
            (*task).wake_at.store(0, Ordering::Relaxed);
            (*task).migrations.store(0, Ordering::Relaxed);
            (*task).ring3_context_id.store(usize::MAX, Ordering::Relaxed);
            (*task).cpu_affine.store(false, Ordering::Relaxed);
            (*task).exit_reason = None;
            (*task).state.store(TaskState::Empty as u8, Ordering::Relaxed);
        }
    }
    if let Err(error) = initialize_fpu_contexts(cpu_index) {
        irq_restore(interrupt_flags);
        return Err(error);
    }
    unsafe {
        let boot = task_ptr(cpu_index, 0);
        (*boot).state.store(TaskState::Running as u8, Ordering::Release);
        for (offset, entry) in entries.iter().copied().enumerate() {
            let task = task_ptr(cpu_index, offset + 1);
            (*task).entry.store(entry as usize, Ordering::Relaxed);
            let rsp = build_initial_context(cpu_index, task);
            (*task).saved_rsp.store(rsp, Ordering::Release);
            (*task).state.store(TaskState::Runnable as u8, Ordering::Release);
        }
    }
    unsafe { (*sched).current_task.store(0, Ordering::Release); }
    unsafe { (*sched).task_count.store(entries.len() + 1, Ordering::Release); }
    unsafe { (*sched).initialized.store(true, Ordering::Release); }
    irq_restore(interrupt_flags);
    Ok(())
}

/// Initialize the current CPU's scheduler (legacy convenience entry point).
pub fn initialize(entries: &[TaskEntry]) -> Result<(), &'static str> {
    initialize_cpu(current_cpu_index(), entries)
}

/// Select the next runnable task in round-robin order on the current CPU.
fn select_next(current: usize, states: &[TaskState]) -> Option<usize> {
    if states.is_empty() || current >= states.len() {
        return None;
    }
    for distance in 1..=states.len() {
        let candidate = (current + distance) % states.len();
        if states[candidate] == TaskState::Runnable {
            return Some(candidate);
        }
    }
    None
}

fn schedule(cpu_index: usize, saved_rsp: usize, count_preemption: bool) -> usize {
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if !unsafe { (*sched).initialized.load(Ordering::Acquire) } { return saved_rsp; }

    lock_run_queue(cpu_index);
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    let current = unsafe { (*sched).current_task.load(Ordering::Relaxed) };
    if current >= count {
        unlock_run_queue(cpu_index);
        return saved_rsp;
    }

    // Interrupt gates enter with IF cleared. The queue lock also excludes remote stealers.
    unsafe {
        let current_task = task_ptr(cpu_index, current);
        let current_state = (*current_task).state.load(Ordering::Acquire);
        if current_state != TaskState::Exited as u8
            && current_state != TaskState::Empty as u8
            && current_state != TaskState::Migrating as u8
        {
            // Blocked tasks must retain the interrupt frame that will resume them
            // after a later wake-up; only a genuinely running task becomes READY.
            (*current_task).saved_rsp.store(saved_rsp, Ordering::Release);
            (*current_task).switches.fetch_add(1, Ordering::Relaxed);
            if current_state == TaskState::Running as u8 {
                (*current_task).state.store(TaskState::Runnable as u8, Ordering::Release);
            }
        }
    }

    let mut states = [TaskState::Empty; MAX_TASKS];
    for index in 0..count {
        states[index] = TaskState::from_raw(unsafe { (*task_ptr(cpu_index, index)).state.load(Ordering::Acquire) });
    }
    let mut next = select_next(current, &states[..count]);
    let only_current_ready = next == Some(current);
    unlock_run_queue(cpu_index);

    // An idle CPU attempts a real transfer before returning to its boot/idle context.
    if only_current_ready || next.is_none() || next == Some(0) {
        // Task slot zero is the CPU's idle/boot context, not useful queued work.
        let _ = steal_task(cpu_index);
    }

    lock_run_queue(cpu_index);
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    let mut states = [TaskState::Empty; MAX_TASKS];
    for index in 0..count {
        states[index] = TaskState::from_raw(unsafe { (*task_ptr(cpu_index, index)).state.load(Ordering::Acquire) });
    }
    next = select_next(current, &states[..count]);
    if let Some(next_index) = next {
        let next_task = unsafe { task_ptr(cpu_index, next_index) };
        let next_rsp = unsafe { (*next_task).saved_rsp.load(Ordering::Acquire) };
        if next_rsp != 0 && unsafe { (*next_task).state.compare_exchange(TaskState::Runnable as u8, TaskState::Running as u8, Ordering::AcqRel, Ordering::Acquire) }.is_ok() {
            if count_preemption && next_index != current {
                unsafe { (*sched).preemption_count.fetch_add(1, Ordering::Relaxed); }
            }
            unsafe { (*sched).current_task.store(next_index, Ordering::Release); }
            unlock_run_queue(cpu_index);
            return next_rsp;
        }
        // If the current task was already RUNNING (e.g. a spurious timer), preserve it.
        if next_index == current && unsafe { (*next_task).state.load(Ordering::Acquire) } == TaskState::Running as u8 {
            unlock_run_queue(cpu_index);
            return saved_rsp;
        }
    }
    unlock_run_queue(cpu_index);
    saved_rsp
}

/// Called by the software-yield interrupt gate. Returns the stack to restore.
pub fn on_yield(saved_rsp: usize) -> usize {
    let cpu_index = current_cpu_index();
    schedule(cpu_index, saved_rsp, false)
}

/// Called by the Local APIC timer interrupt when automatic preemption is enabled.
pub fn on_timer(saved_rsp: usize) -> usize {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if !unsafe { (*sched).preemptive_enabled.load(Ordering::Acquire) } {
        return saved_rsp;
    }
    wake_sleepers(cpu_index, crate::clock::ticks());
    schedule(cpu_index, saved_rsp, true)
}

/// Wake every blocked task whose monotonic deadline has elapsed on the given CPU.
pub fn wake_sleepers(cpu_index: usize, now: u64) -> usize {
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if !unsafe { (*sched).initialized.load(Ordering::Acquire) } { return 0; }
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    let mut woke = 0;
    // SAFETY: Timer interrupt runs with IF cleared and this scheduler is per-CPU.
    unsafe {
        for index in 1..count {
            let task = task_ptr(cpu_index, index);
            if (*task).state.load(Ordering::Acquire) == TaskState::Blocked as u8
                && (*task).wake_at.load(Ordering::Acquire) != 0
                && (*task).wake_at.load(Ordering::Acquire) <= now {
                (*task).wake_at.store(0, Ordering::Relaxed);
                (*task).state.store(TaskState::Runnable as u8, Ordering::Release);
                woke += 1;
            }
        }
    }
    woke
}

/// Block the current non-boot task until a monotonic tick deadline.
/// Returns false when no alternate runnable task exists.
pub fn sleep_for_ticks(ticks: u64) -> bool {
    if ticks == 0 { yield_now(); return true; }
    let cpu_index = current_cpu_index();
    let flags = irq_save_disable();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if !unsafe { (*sched).initialized.load(Ordering::Acquire) } {
        irq_restore(flags);
        return false;
    }
    let current = unsafe { (*sched).current_task.load(Ordering::Acquire) };
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    if current == 0 || current >= count {
        irq_restore(flags);
        return false;
    }
    let mut alternate_runnable = false;
    // SAFETY: Interrupts are disabled while checking and changing scheduler state.
    unsafe {
        for index in 0..count {
            if index != current
                && (*task_ptr(cpu_index, index)).state.load(Ordering::Acquire) == TaskState::Runnable as u8 {
                alternate_runnable = true;
                break;
            }
        }
        if !alternate_runnable {
            irq_restore(flags);
            return false;
        }
        let task = task_ptr(cpu_index, current);
        let deadline = crate::clock::ticks().saturating_add(ticks);
        (*task).wake_at.store(deadline, Ordering::Release);
        (*task).state.store(TaskState::Blocked as u8, Ordering::Release);
    }
    irq_restore(flags);
    yield_now();
    true
}

/// Block the current non-boot task until another task explicitly wakes it.
pub fn block_current() -> bool {
    let cpu_index = current_cpu_index();
    let flags = irq_save_disable();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if !unsafe { (*sched).initialized.load(Ordering::Acquire) } {
        irq_restore(flags);
        return false;
    }
    let current = unsafe { (*sched).current_task.load(Ordering::Acquire) };
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    if current == 0 || current >= count {
        irq_restore(flags);
        return false;
    }
    let mut alternate_runnable = false;
    // SAFETY: Interrupts are disabled while checking and changing scheduler state.
    unsafe {
        for index in 0..count {
            if index != current
                && (*task_ptr(cpu_index, index)).state.load(Ordering::Acquire) == TaskState::Runnable as u8 {
                alternate_runnable = true;
                break;
            }
        }
        if !alternate_runnable {
            irq_restore(flags);
            return false;
        }
        let task = task_ptr(cpu_index, current);
        (*task).wake_at.store(0, Ordering::Release);
        (*task).state.store(TaskState::Blocked as u8, Ordering::Release);
    }
    irq_restore(flags);
    yield_now();
    true
}

/// Explicitly wake a blocked task on the current CPU, for event completion or synchronization.
pub fn wake_task(id: TaskId) -> bool {
    let cpu_index = current_cpu_index();
    let flags = irq_save_disable();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if !unsafe { (*sched).initialized.load(Ordering::Acquire) } || id.0 == 0 || id.0 >= unsafe { (*sched).task_count.load(Ordering::Acquire) } {
        irq_restore(flags);
        return false;
    }
    // SAFETY: Index is validated and interrupts are disabled for the state transition.
    let task = unsafe { task_ptr(cpu_index, id.0) };
    let blocked = unsafe { (*task).state.load(Ordering::Acquire) == TaskState::Blocked as u8 };
    if blocked {
        unsafe {
            (*task).wake_at.store(0, Ordering::Relaxed);
            (*task).state.store(TaskState::Runnable as u8, Ordering::Release);
        }
    }
    irq_restore(flags);
    blocked
}

/// Enable timer-driven preemption after all task contexts are initialized on the current CPU.
pub fn enable_preemption() {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    unsafe { (*sched).preemptive_enabled.store(true, Ordering::Release); }
}

/// Number of timer interrupts that switched to a different runnable task on the current CPU.
pub fn preemption_count() -> usize {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    unsafe { (*sched).preemption_count.load(Ordering::Acquire) }
}

fn irq_save_disable() -> usize {
    let flags: usize;
    // SAFETY: RFLAGS is readable and CLI is valid at ring 0.
    unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) flags, options(nomem)); }
    flags
}

fn irq_restore(flags: usize) {
    if flags & (1 << 9) != 0 {
        // SAFETY: Restore the prior IF state after the scheduler state transition.
        unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
    }
}

/// Reset the scheduler after every non-boot task has exited on the current CPU.
pub fn reset_after_all_tasks_exit() -> bool {
    let cpu_index = current_cpu_index();
    let flags = irq_save_disable();
    lock_run_queue(cpu_index);
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if !unsafe { (*sched).initialized.load(Ordering::Acquire) } || unsafe { (*sched).current_task.load(Ordering::Acquire) } != 0 {
        unlock_run_queue(cpu_index);
        irq_restore(flags);
        return false;
    }
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    for index in 1..count {
        // SAFETY: Index is bounded by TASK_COUNT and interrupts are disabled.
        let state = unsafe { (*task_ptr(cpu_index, index)).state.load(Ordering::Acquire) };
        if state != TaskState::Exited as u8 && state != TaskState::Empty as u8 {
            unlock_run_queue(cpu_index);
            irq_restore(flags);
            return false;
        }
    }
    unsafe { (*sched).preemptive_enabled.store(false, Ordering::Release); }
    unsafe { (*sched).preemption_count.store(0, Ordering::Release); }
    unsafe { (*sched).task_count.store(1, Ordering::Release); }
    unsafe { (*sched).current_task.store(0, Ordering::Release); }
    unsafe { (*sched).initialized.store(false, Ordering::Release); }
    unlock_run_queue(cpu_index);
    irq_restore(flags);
    true
}

/// Yield voluntarily to the next runnable kernel task on the current CPU.
pub fn yield_now() {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if unsafe { (*sched).initialized.load(Ordering::Acquire) } {
        // SAFETY: IDT vector 0x41 is installed as a ring-0 interrupt gate during boot.
        unsafe { core::arch::asm!("int 0x41", options(nomem, nostack)); }
    }
}

/// Enter the per-CPU scheduler loop.
/// This function is called by each CPU (BSP and APs) after initialization.
/// It runs the scheduler on the current CPU until all tasks exit.
pub fn run_cpu_loop() -> ! {
    // Enable interrupts and enter the idle loop.
    // Each CPU will handle its own timer interrupts and schedule tasks.
    unsafe { core::arch::asm!("sti", options(nomem, nostack)) };
    loop {
        // SAFETY: The scheduler runs with interrupts enabled; HLT waits for the next IRQ.
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    }
}

pub fn current_task() -> TaskId {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    TaskId(unsafe { (*sched).current_task.load(Ordering::Acquire) })
}

/// Return the current task only when the local scheduler and slot are valid.
pub fn current_task_id() -> Option<TaskId> {
    let cpu_index = current_cpu_index();
    if cpu_index >= MAX_CPUS { return None; }
    let sched = cpu_scheduler(cpu_index);
    if !unsafe { (*sched).initialized.load(Ordering::Acquire) } { return None; }
    let current = unsafe { (*sched).current_task.load(Ordering::Acquire) };
    if current >= unsafe { (*sched).task_count.load(Ordering::Acquire) } { return None; }
    Some(TaskId(current))
}

/// Attach an owner-managed Ring 3 context to an initialized, non-current task slot.
pub fn attach_ring3_context(task_id: TaskId, context: Ring3TaskContextRef) -> Result<(), SchedulerError> {
    let cpu_index = current_cpu_index();
    if cpu_index >= MAX_CPUS { return Err(SchedulerError::InvalidTask); }
    let sched = cpu_scheduler(cpu_index);
    if !unsafe { (*sched).initialized.load(Ordering::Acquire) } { return Err(SchedulerError::NotInitialized); }
    lock_run_queue(cpu_index);
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    let current = unsafe { (*sched).current_task.load(Ordering::Acquire) };
    if task_id.0 == 0 {
        unlock_run_queue(cpu_index);
        return Err(SchedulerError::BootTask);
    }
    if task_id.0 >= count {
        unlock_run_queue(cpu_index);
        return Err(SchedulerError::InvalidTask);
    }
    if task_id.0 == current {
        unlock_run_queue(cpu_index);
        return Err(SchedulerError::CurrentTask);
    }
    let task = task_ptr(cpu_index, task_id.0);
    if unsafe { (*task).state.load(Ordering::Acquire) } != TaskState::Runnable as u8
        || unsafe { (*task).ring3_context_id.load(Ordering::Acquire) } != usize::MAX
    {
        unlock_run_queue(cpu_index);
        return Err(SchedulerError::ContextAlreadyAttached);
    }
    // Affinity is published before the opaque context ID, while thief and attach
    // serialize on the same run-queue lock.
    unsafe {
        (*task).cpu_affine.store(true, Ordering::Release);
        (*task).ring3_context_id.store(context.0, Ordering::Release);
    }
    unlock_run_queue(cpu_index);
    Ok(())
}

/// Record the first exit reason for the currently running task.
pub fn mark_current_exited(reason: TaskExitReason) -> Result<(), SchedulerError> {
    let cpu_index = current_cpu_index();
    if cpu_index >= MAX_CPUS { return Err(SchedulerError::InvalidTask); }
    let sched = cpu_scheduler(cpu_index);
    if !unsafe { (*sched).initialized.load(Ordering::Acquire) } { return Err(SchedulerError::NotInitialized); }
    lock_run_queue(cpu_index);
    let current = unsafe { (*sched).current_task.load(Ordering::Acquire) };
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    if current == 0 || current >= count {
        unlock_run_queue(cpu_index);
        return Err(SchedulerError::InvalidTask);
    }
    let task = task_ptr(cpu_index, current);
    // SAFETY: The queue lock serializes the sole write and state publication.
    let marked = unsafe { (*task).try_mark_exited(reason) };
    unlock_run_queue(cpu_index);
    if marked { Ok(()) } else { Err(SchedulerError::AlreadyExited) }
}

/// Select another runnable task after the caller has already published Exited.
/// This path never saves or reactivates the terminating task and frees no resources.
pub fn schedule_after_exit(saved_rsp: usize) -> usize {
    let cpu_index = current_cpu_index();
    if cpu_index >= MAX_CPUS { return saved_rsp; }
    let sched = cpu_scheduler(cpu_index);
    if !unsafe { (*sched).initialized.load(Ordering::Acquire) } { return saved_rsp; }
    lock_run_queue(cpu_index);
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    let current = unsafe { (*sched).current_task.load(Ordering::Acquire) };
    if current >= count
        || unsafe { (*task_ptr(cpu_index, current)).state.load(Ordering::Acquire) } != TaskState::Exited as u8
    {
        unlock_run_queue(cpu_index);
        return saved_rsp;
    }
    let mut states = [TaskState::Empty; MAX_TASKS];
    for index in 0..count {
        states[index] = TaskState::from_raw(unsafe { (*task_ptr(cpu_index, index)).state.load(Ordering::Acquire) });
    }
    let next = select_next(current, &states[..count]);
    if let Some(next_index) = next {
        let next_task = task_ptr(cpu_index, next_index);
        let next_rsp = unsafe { (*next_task).saved_rsp.load(Ordering::Acquire) };
        if next_rsp != 0
            && unsafe { (*next_task).state.compare_exchange(TaskState::Runnable as u8, TaskState::Running as u8, Ordering::AcqRel, Ordering::Acquire) }.is_ok()
        {
            unsafe { (*sched).current_task.store(next_index, Ordering::Release); }
            unlock_run_queue(cpu_index);
            return next_rsp;
        }
    }
    unlock_run_queue(cpu_index);
    saved_rsp
}

pub fn task_state(id: TaskId) -> Option<TaskState> {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if id.0 >= unsafe { (*sched).task_count.load(Ordering::Acquire) } {
        return None;
    }
    // SAFETY: Task table is static and the index is bounded by TASK_COUNT.
    Some(TaskState::from_raw(unsafe { (*task_ptr(cpu_index, id.0)).state.load(Ordering::Acquire) }))
}

pub fn switch_count(id: TaskId) -> Option<usize> {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    if id.0 >= unsafe { (*sched).task_count.load(Ordering::Acquire) } {
        return None;
    }
    // SAFETY: Task table is static and the index is bounded by TASK_COUNT.
    Some(unsafe { (*task_ptr(cpu_index, id.0)).switches.load(Ordering::Acquire) })
}

/// Validate synthetic IRETQ frames before running the first task on the current CPU.
pub fn validate_initial_contexts() -> bool {
    let cpu_index = current_cpu_index();
    let sched = unsafe { cpu_scheduler(cpu_index) };
    let count = unsafe { (*sched).task_count.load(Ordering::Acquire) };
    for index in 1..count {
        // SAFETY: Each saved pointer was built from a live static task stack.
        let rsp = unsafe { (*task_ptr(cpu_index, index)).saved_rsp.load(Ordering::Acquire) };
        if rsp == 0 { return false; }
        let frame = rsp as *const usize;
        // SAFETY: The frame spans INTERRUPT_FRAME_WORDS words inside its static task stack.
        let valid = unsafe {
            frame.add(15).read() == YIELD_VECTOR
                && frame.add(16).read() == 0
                && frame.add(17).read() == task_bootstrap as *const () as usize
                && frame.add(18).read() == KERNEL_CODE_SELECTOR
                && frame.add(19).read() == INITIAL_RFLAGS
                && frame.add(20).read() != 0
                && frame.add(21).read() == KERNEL_DATA_SELECTOR
        };
        if !valid { return false; }
    }
    true
}

#[cfg(feature = "qemu-cooperative-scheduler-test")]
pub fn run_cooperative_qemu_test() -> bool {
    qemu_test::run()
}

#[cfg(feature = "qemu-cooperative-scheduler-test")]
mod qemu_test {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    static COUNTS: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];

    extern "C" fn worker_a() { for _ in 0..5 { COUNTS[0].fetch_add(1, Ordering::Relaxed); yield_now(); } }
    extern "C" fn worker_b() { for _ in 0..5 { COUNTS[1].fetch_add(1, Ordering::Relaxed); yield_now(); } }
    extern "C" fn worker_c() { for _ in 0..5 { COUNTS[2].fetch_add(1, Ordering::Relaxed); yield_now(); } }

    pub fn run() -> bool {
        for count in &COUNTS { count.store(0, Ordering::Relaxed); }
        if initialize(&[worker_a, worker_b, worker_c]).is_err() || !validate_initial_contexts() {
            return false;
        }
        loop {
            let all_exited = (1..=3).all(|index| task_state(TaskId(index)) == Some(TaskState::Exited));
            if all_exited { break; }
            yield_now();
        }
        let valid = current_task() == TaskId(0)
            && COUNTS.iter().all(|count| count.load(Ordering::Acquire) == 5)
            && (1..=3).all(|index| switch_count(TaskId(index)).unwrap_or(0) > 0);
        reset_after_all_tasks_exit() && valid
    }
}

#[cfg(feature = "qemu-fpu-context-test")]
mod fpu_context_qemu_test {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    const RUNTIME_TICKS: u64 = 5;
    static RESULTS: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];
    static SIMD_RESULTS: [[AtomicUsize; 2]; 3] = [
        [const { AtomicUsize::new(0) }; 2],
        [const { AtomicUsize::new(0) }; 2],
        [const { AtomicUsize::new(0) }; 2],
    ];

    fn hold_fpu_and_simd_state(index: usize, x87_bits: u64, simd: [u64; 2]) {
        let start = crate::clock::ticks();
        let tick_address = crate::clock::tick_counter_address();
        let mut x87_result = 0u64;
        let mut simd_result = [0u64; 2];
        // The entire wait is assembly: XMM15 and x87 ST(0) remain live across hardware IRQs.
        unsafe {
            core::arch::asm!(
                "fld qword ptr [{x87}]",
                "movdqu xmm15, xmmword ptr [{simd}]",
                "2:",
                "mov rax, qword ptr [{ticks}]",
                "sub rax, {start}",
                "cmp rax, {duration}",
                "jb 2b",
                "fstp qword ptr [{x87_out}]",
                "movdqu xmmword ptr [{simd_out}], xmm15",
                x87 = in(reg) &x87_bits,
                simd = in(reg) &simd,
                ticks = in(reg) tick_address,
                start = in(reg) start,
                duration = in(reg) RUNTIME_TICKS,
                x87_out = in(reg) &mut x87_result,
                simd_out = in(reg) &mut simd_result,
                out("rax") _,
                out("xmm15") _,
                options(nostack)
            );
        }
        RESULTS[index].store(x87_result as usize, Ordering::Release);
        SIMD_RESULTS[index][0].store(simd_result[0] as usize, Ordering::Release);
        SIMD_RESULTS[index][1].store(simd_result[1] as usize, Ordering::Release);
    }

    extern "C" fn worker_a() { hold_fpu_and_simd_state(0, 0x3FF4_0000_0000_0000, [0x1111_2222_3333_4444, 0xAAAA_BBBB_CCCC_DDDD]); }
    extern "C" fn worker_b() { hold_fpu_and_simd_state(1, 0x4004_0000_0000_0000, [0x5555_6666_7777_8888, 0xEEEE_FFFF_0000_1111]); }
    extern "C" fn worker_c() { hold_fpu_and_simd_state(2, 0x400E_0000_0000_0000, [0x9999_AAAA_BBBB_CCCC, 0x2222_3333_4444_5555]); }

    pub fn run() -> bool {
        for result in &RESULTS { result.store(0, Ordering::Relaxed); }
        for pair in &SIMD_RESULTS { pair[0].store(0, Ordering::Relaxed); pair[1].store(0, Ordering::Relaxed); }
        if initialize(&[worker_a, worker_b, worker_c]).is_err() || !validate_initial_contexts() {
            return false;
        }
        enable_preemption();
        loop {
            let all_exited = (1..=3).all(|index| task_state(TaskId(index)) == Some(TaskState::Exited));
            if all_exited { break; }
            core::hint::spin_loop();
        }
        let expected_x87 = [0x3FF4_0000_0000_0000usize, 0x4004_0000_0000_0000usize, 0x400E_0000_0000_0000usize];
        let expected_simd = [
            [0x1111_2222_3333_4444usize, 0xAAAA_BBBB_CCCC_DDDDusize],
            [0x5555_6666_7777_8888usize, 0xEEEE_FFFF_0000_1111usize],
            [0x9999_AAAA_BBBB_CCCCusize, 0x2222_3333_4444_5555usize],
        ];
        let valid = current_task() == TaskId(0)
            && (0..3).all(|i| RESULTS[i].load(Ordering::Acquire) == expected_x87[i])
            && (0..3).all(|i| (0..2).all(|j| SIMD_RESULTS[i][j].load(Ordering::Acquire) == expected_simd[i][j]))
            && preemption_count() >= 3;
        reset_after_all_tasks_exit() && valid
    }
}

#[cfg(feature = "qemu-fpu-context-test")]
pub fn run_fpu_context_qemu_test() -> bool {
    fpu_context_qemu_test::run()
}

#[cfg(feature = "qemu-task-sleep-test")]
mod task_sleep_qemu_test {
    use super::*;
    use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    static SLEEP_STARTED: AtomicU64 = AtomicU64::new(0);
    static SLEEP_FINISHED: AtomicU64 = AtomicU64::new(0);
    static OBSERVED_BLOCKED: AtomicUsize = AtomicUsize::new(0);
    static WORKER_DONE: AtomicUsize = AtomicUsize::new(0);
    static EVENT_WAKE_DONE: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn sleeper() {
        SLEEP_STARTED.store(crate::clock::ticks(), Ordering::Release);
        if sleep_for_ticks(4) {
            SLEEP_FINISHED.store(crate::clock::ticks(), Ordering::Release);
        }
    }

    extern "C" fn observer() {
        for _ in 0..8 {
            if task_state(TaskId(1)) == Some(TaskState::Blocked) {
                OBSERVED_BLOCKED.fetch_add(1, Ordering::Relaxed);
            }
            core::hint::spin_loop();
        }
        WORKER_DONE.store(1, Ordering::Release);
    }

    extern "C" fn event_waiter() {
        if block_current() { EVENT_WAKE_DONE.fetch_add(1, Ordering::AcqRel); }
    }

    extern "C" fn event_waker() {
        while task_state(TaskId(3)) != Some(TaskState::Blocked) { core::hint::spin_loop(); }
        if wake_task(TaskId(3)) { EVENT_WAKE_DONE.fetch_add(1, Ordering::AcqRel); }
    }

    pub fn run() -> bool {
        SLEEP_STARTED.store(0, Ordering::Relaxed);
        SLEEP_FINISHED.store(0, Ordering::Relaxed);
        OBSERVED_BLOCKED.store(0, Ordering::Relaxed);
        WORKER_DONE.store(0, Ordering::Relaxed);
        EVENT_WAKE_DONE.store(0, Ordering::Relaxed);
        if initialize(&[sleeper, observer, event_waiter, event_waker]).is_err() || !validate_initial_contexts() { return false; }
        enable_preemption();
        loop {
            let done = (1..=4).all(|index| task_state(TaskId(index)) == Some(TaskState::Exited));
            if done { break; }
            core::hint::spin_loop();
        }
        let started = SLEEP_STARTED.load(Ordering::Acquire);
        let finished = SLEEP_FINISHED.load(Ordering::Acquire);
        let valid = started != 0
            && finished.wrapping_sub(started) >= 4
            && OBSERVED_BLOCKED.load(Ordering::Acquire) > 0
            && WORKER_DONE.load(Ordering::Acquire) == 1
            && task_state(TaskId(1)) == Some(TaskState::Exited)
            && EVENT_WAKE_DONE.load(Ordering::Acquire) == 2;
        reset_after_all_tasks_exit() && valid
    }
}

#[cfg(feature = "qemu-task-sleep-test")]
pub fn run_task_sleep_qemu_test() -> bool {
    task_sleep_qemu_test::run()
}

#[cfg(feature = "qemu-preemptive-scheduler-test")]
mod preemptive_qemu_test {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    const TASK_RUNTIME_TICKS: u64 = 5;
    static COMPLETED: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];

    fn consume_five_timer_ticks() {
        let start = crate::clock::ticks();
        while crate::clock::ticks().wrapping_sub(start) < TASK_RUNTIME_TICKS {
            core::hint::spin_loop();
        }
    }

    // These entries never call yield_now; only the Local APIC timer can switch them.
    extern "C" fn worker_a() {
        consume_five_timer_ticks();
        COMPLETED[0].fetch_add(1, Ordering::Release);
    }
    extern "C" fn worker_b() {
        consume_five_timer_ticks();
        COMPLETED[1].fetch_add(1, Ordering::Release);
    }
    extern "C" fn worker_c() {
        consume_five_timer_ticks();
        COMPLETED[2].fetch_add(1, Ordering::Release);
    }

    pub fn run() -> bool {
        for completed in &COMPLETED { completed.store(0, Ordering::Relaxed); }
        if initialize(&[worker_a, worker_b, worker_c]).is_err() || !validate_initial_contexts() {
            return false;
        }
        enable_preemption();
        loop {
            let all_exited = (1..=3).all(|index| task_state(TaskId(index)) == Some(TaskState::Exited));
            if all_exited { break; }
            core::hint::spin_loop();
        }
        let valid = current_task() == TaskId(0)
            && COMPLETED.iter().all(|count| count.load(Ordering::Acquire) == 1)
            && (1..=3).all(|index| switch_count(TaskId(index)).unwrap_or(0) > 0)
            && preemption_count() >= 3;
        reset_after_all_tasks_exit() && valid
    }
}

#[cfg(feature = "qemu-preemptive-scheduler-test")]
pub fn run_preemptive_qemu_test() -> bool {
    preemptive_qemu_test::run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_exit_reason_defaults_to_none_for_existing_kernel_tasks() {
        let task = Task::new();
        assert_eq!(task.exit_reason, None);
    }

    #[test]
    fn task_exit_reason_preserves_every_fixed_size_variant() {
        let cases = [
            TaskExitReason::VoluntaryExit { code: 23 },
            TaskExitReason::PageFault { cr2: 0x1234_5000, error_code: 5, rip: 0x4000 },
            TaskExitReason::GeneralProtection { error_code: 0, rip: 0x5000 },
        ];

        for reason in cases {
            let mut task = Task::new();
            task.state.store(TaskState::Running as u8, Ordering::Relaxed);
            assert!(task.try_mark_exited(reason));
            assert_eq!(task.exit_reason, Some(reason));
            assert_eq!(TaskState::from_raw(task.state.load(Ordering::Acquire)), TaskState::Exited);
        }
    }

    #[test]
    fn double_termination_does_not_overwrite_the_first_exit_reason() {
        let mut task = Task::new();
        task.state.store(TaskState::Running as u8, Ordering::Relaxed);
        let first = TaskExitReason::VoluntaryExit { code: 7 };
        let second = TaskExitReason::PageFault { cr2: 0xdead_0000, error_code: 4, rip: 0x6000 };

        assert!(task.try_mark_exited(first));
        assert!(!task.try_mark_exited(second));
        assert_eq!(task.exit_reason, Some(first));
    }

    #[test]
    fn blocked_task_is_woken_only_at_or_after_its_deadline() {
        let states = [TaskState::Runnable, TaskState::Blocked, TaskState::Runnable];
        assert_eq!(select_next(0, &states), Some(2));
    }

    #[test]
    fn fpu_context_image_has_fxsave_size_and_alignment() {
        assert_eq!(core::mem::size_of::<FpuState>(), 512);
        assert_eq!(core::mem::align_of::<FpuState>(), 16);
    }

    #[test]
    fn task_states_have_stable_kernel_values() {
        assert_eq!(TaskState::from_raw(1), TaskState::Runnable);
        assert_eq!(TaskState::from_raw(2), TaskState::Blocked);
        assert_eq!(TaskState::from_raw(3), TaskState::Exited);
        assert_eq!(TaskState::from_raw(4), TaskState::Running);
        assert_eq!(TaskState::from_raw(5), TaskState::Migrating);
        assert_eq!(TaskState::from_raw(255), TaskState::Empty);
    }

    #[test]
    fn round_robin_skips_blocked_and_exited_tasks() {
        let states = [TaskState::Runnable, TaskState::Blocked, TaskState::Exited, TaskState::Runnable];
        assert_eq!(select_next(0, &states), Some(3));
        assert_eq!(select_next(3, &states), Some(0));
    }

    #[test]
    fn round_robin_rejects_an_empty_or_invalid_task_set() {
        assert_eq!(select_next(0, &[]), None);
        assert_eq!(select_next(4, &[TaskState::Runnable]), None);
        assert_eq!(select_next(0, &[TaskState::Blocked, TaskState::Exited]), None);
    }
}