//! Single-core kernel task scheduler for x86_64.
//!
//! Tasks share the kernel address space and are switched by cooperative yield or
//! Local APIC timer preemption. Legacy x87/MMX/SSE state is eagerly isolated per task.

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering};

pub const MAX_TASKS: usize = 8;
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
}

impl TaskState {
    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Runnable,
            2 => Self::Blocked,
            3 => Self::Exited,
            _ => Self::Empty,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskId(pub usize);

pub type TaskEntry = extern "C" fn();

#[repr(C, align(16))]
struct FpuState([u8; FPU_STATE_SIZE]);

#[repr(align(16))]
struct TaskStack([u8; STACK_SIZE]);

struct Task {
    stack: TaskStack,
    saved_rsp: AtomicUsize,
    state: AtomicU8,
    switches: AtomicUsize,
    entry: AtomicUsize,
    fpu: FpuState,
    wake_at: AtomicU64,
}

impl Task {
    const fn new() -> Self {
        Self {
            stack: TaskStack([0; STACK_SIZE]),
            saved_rsp: AtomicUsize::new(0),
            state: AtomicU8::new(TaskState::Empty as u8),
            switches: AtomicUsize::new(0),
            entry: AtomicUsize::new(0),
            fpu: FpuState([0; FPU_STATE_SIZE]),
            wake_at: AtomicU64::new(0),
        }
    }
}

static mut TASKS: [Task; MAX_TASKS] = [const { Task::new() }; MAX_TASKS];
static TASK_COUNT: AtomicUsize = AtomicUsize::new(1);
static CURRENT_TASK: AtomicUsize = AtomicUsize::new(0);
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static PREEMPTIVE_ENABLED: AtomicBool = AtomicBool::new(false);
static PREEMPTION_COUNT: AtomicUsize = AtomicUsize::new(0);
static FPU_CONTEXTS_READY: AtomicBool = AtomicBool::new(false);


/// Configure architectural x87/SSE state management and initialize every task image.
fn initialize_fpu_contexts() -> Result<(), &'static str> {
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
    let boot_ptr = unsafe { core::ptr::addr_of_mut!((*task_ptr(0)).fpu.0).cast::<u8>() };
    let mxcsr: u32 = 0x1F80;
    // SAFETY: FPU state buffers are 16-byte aligned and at least 512 bytes long.
    unsafe {
        core::arch::asm!("fxsave64 [{}]", in(reg) boot_ptr, options(nostack));
        core::arch::asm!("fninit", "ldmxcsr [{}]", in(reg) &mxcsr, options(nostack));
        for index in 1..MAX_TASKS {
            let state = core::ptr::addr_of_mut!((*task_ptr(index)).fpu.0).cast::<u8>();
            core::arch::asm!("fxsave64 [{}]", in(reg) state, options(nostack));
        }
        core::arch::asm!("fxrstor64 [{}]", in(reg) boot_ptr, options(nostack));
    }
    FPU_CONTEXTS_READY.store(true, Ordering::Release);
    Ok(())
}

/// Save the interrupted task's complete legacy x87/MMX/SSE image before Rust IRQ code runs.
#[unsafe(no_mangle)]
pub extern "C" fn systemes_fpu_save_current() {
    if !FPU_CONTEXTS_READY.load(Ordering::Acquire) {
        return;
    }
    let current = CURRENT_TASK.load(Ordering::Acquire);
    if current >= TASK_COUNT.load(Ordering::Acquire) {
        return;
    }
    // SAFETY: The selected task owns a 16-byte-aligned 512-byte FXSAVE image.
    unsafe {
        let state = core::ptr::addr_of_mut!((*task_ptr(current)).fpu.0).cast::<u8>();
        core::arch::asm!("fxsave64 [{}]", in(reg) state, options(nostack));
    }
}

/// Restore the selected task's FPU image after Rust IRQ dispatch and before IRETQ.
#[unsafe(no_mangle)]
pub extern "C" fn systemes_fpu_restore_current() {
    if !FPU_CONTEXTS_READY.load(Ordering::Acquire) {
        return;
    }
    let current = CURRENT_TASK.load(Ordering::Acquire);
    if current >= TASK_COUNT.load(Ordering::Acquire) {
        return;
    }
    // SAFETY: The selected task owns a previously initialized 16-byte-aligned FXSAVE image.
    unsafe {
        let state = core::ptr::addr_of!((*task_ptr(current)).fpu.0).cast::<u8>();
        core::arch::asm!("fxrstor64 [{}]", in(reg) state, options(nostack));
    }
}

unsafe fn task_ptr(index: usize) -> *mut Task {
    // SAFETY: Callers validate index against TASK_COUNT/MAX_TASKS. The task table
    // is static and cooperative state changes occur on one CPU in interrupt gates.
    unsafe { core::ptr::addr_of_mut!(TASKS).cast::<Task>().add(index) }
}

fn build_initial_context(task: *mut Task) -> usize {
    // SAFETY: The task is exclusively initialized before it becomes runnable.
    unsafe {
        let stack = core::ptr::addr_of_mut!((*task).stack.0).cast::<u8>();
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
    let current = CURRENT_TASK.load(Ordering::Acquire);
    if current >= TASK_COUNT.load(Ordering::Acquire) {
        task_returned();
    }
    // SAFETY: The current task index is bounded and its entry was stored before it became runnable.
    let entry_address = unsafe { (*task_ptr(current)).entry.load(Ordering::Acquire) };
    if entry_address == 0 {
        task_returned();
    }
    // SAFETY: initialize stores a valid extern "C" TaskEntry function pointer as usize.
    let entry: TaskEntry = unsafe { core::mem::transmute(entry_address) };
    entry();
    // SAFETY: The current task is the only writer of its exit state on this single CPU.
    unsafe { (*task_ptr(current)).state.store(TaskState::Exited as u8, Ordering::Release); }
    loop {
        yield_now();
    }
}

/// Initialize the boot task and up to seven cooperative kernel tasks.
/// Call once during boot before task entries are allowed to run.
pub fn initialize(entries: &[TaskEntry]) -> Result<(), &'static str> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Err("scheduler already initialized");
    }
    if entries.len() >= MAX_TASKS {
        return Err("too many kernel tasks");
    }
    initialize_fpu_contexts()?;
    // SAFETY: Called during single-core boot before any task can access the table.
    unsafe {
        for index in 0..MAX_TASKS {
            let task = task_ptr(index);
            (*task).saved_rsp.store(0, Ordering::Relaxed);
            (*task).switches.store(0, Ordering::Relaxed);
            (*task).entry.store(0, Ordering::Relaxed);
            (*task).wake_at.store(0, Ordering::Relaxed);
            (*task).state.store(TaskState::Empty as u8, Ordering::Relaxed);
        }
        let boot = task_ptr(0);
        (*boot).state.store(TaskState::Runnable as u8, Ordering::Release);
        for (offset, entry) in entries.iter().copied().enumerate() {
            let task = task_ptr(offset + 1);
            (*task).entry.store(entry as usize, Ordering::Relaxed);
            let rsp = build_initial_context(task);
            (*task).saved_rsp.store(rsp, Ordering::Release);
            (*task).state.store(TaskState::Runnable as u8, Ordering::Release);
        }
    }
    CURRENT_TASK.store(0, Ordering::Release);
    TASK_COUNT.store(entries.len() + 1, Ordering::Release);
    INITIALIZED.store(true, Ordering::Release);
    Ok(())
}

/// Select the next runnable task in round-robin order.
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

fn schedule(saved_rsp: usize, count_preemption: bool) -> usize {
    if !INITIALIZED.load(Ordering::Acquire) {
        return saved_rsp;
    }
    let count = TASK_COUNT.load(Ordering::Acquire);
    let current = CURRENT_TASK.load(Ordering::Relaxed);
    if current >= count {
        return saved_rsp;
    }

    // SAFETY: Both software-yield and timer gates clear IF; scheduler state is single-core.
    unsafe {
        let current_task = task_ptr(current);
        if (*current_task).state.load(Ordering::Acquire) != TaskState::Exited as u8 {
            (*current_task).saved_rsp.store(saved_rsp, Ordering::Release);
            (*current_task).switches.fetch_add(1, Ordering::Relaxed);
        }

        let mut states = [TaskState::Empty; MAX_TASKS];
        for index in 0..count {
            states[index] = TaskState::from_raw((*task_ptr(index)).state.load(Ordering::Acquire));
        }
        if let Some(next) = select_next(current, &states[..count]) {
            let next_task = task_ptr(next);
            let next_rsp = (*next_task).saved_rsp.load(Ordering::Acquire);
            if next_rsp != 0 {
                if count_preemption && next != current {
                    PREEMPTION_COUNT.fetch_add(1, Ordering::Relaxed);
                }
                CURRENT_TASK.store(next, Ordering::Release);
                return next_rsp;
            }
        }
    }
    saved_rsp
}

/// Called by the software-yield interrupt gate. Returns the stack to restore.
pub fn on_yield(saved_rsp: usize) -> usize {
    schedule(saved_rsp, false)
}

/// Called by the Local APIC timer interrupt when automatic preemption is enabled.
pub fn on_timer(saved_rsp: usize) -> usize {
    if !PREEMPTIVE_ENABLED.load(Ordering::Acquire) {
        return saved_rsp;
    }
    wake_sleepers(crate::clock::ticks());
    schedule(saved_rsp, true)
}

/// Wake every blocked task whose monotonic deadline has elapsed.
pub fn wake_sleepers(now: u64) -> usize {
    if !INITIALIZED.load(Ordering::Acquire) { return 0; }
    let count = TASK_COUNT.load(Ordering::Acquire);
    let mut woke = 0;
    // SAFETY: Timer interrupt runs with IF cleared and this scheduler is single-core.
    unsafe {
        for index in 1..count {
            let task = task_ptr(index);
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
    let flags = irq_save_disable();
    if !INITIALIZED.load(Ordering::Acquire) {
        irq_restore(flags);
        return false;
    }
    let current = CURRENT_TASK.load(Ordering::Acquire);
    let count = TASK_COUNT.load(Ordering::Acquire);
    if current == 0 || current >= count {
        irq_restore(flags);
        return false;
    }
    let mut alternate_runnable = false;
    // SAFETY: Interrupts are disabled while checking and changing scheduler state.
    unsafe {
        for index in 0..count {
            if index != current
                && (*task_ptr(index)).state.load(Ordering::Acquire) == TaskState::Runnable as u8 {
                alternate_runnable = true;
                break;
            }
        }
        if !alternate_runnable {
            irq_restore(flags);
            return false;
        }
        let task = task_ptr(current);
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
    let flags = irq_save_disable();
    if !INITIALIZED.load(Ordering::Acquire) {
        irq_restore(flags);
        return false;
    }
    let current = CURRENT_TASK.load(Ordering::Acquire);
    let count = TASK_COUNT.load(Ordering::Acquire);
    if current == 0 || current >= count {
        irq_restore(flags);
        return false;
    }
    let mut alternate_runnable = false;
    // SAFETY: Interrupts are disabled while checking and changing scheduler state.
    unsafe {
        for index in 0..count {
            if index != current
                && (*task_ptr(index)).state.load(Ordering::Acquire) == TaskState::Runnable as u8 {
                alternate_runnable = true;
                break;
            }
        }
        if !alternate_runnable {
            irq_restore(flags);
            return false;
        }
        let task = task_ptr(current);
        (*task).wake_at.store(0, Ordering::Release);
        (*task).state.store(TaskState::Blocked as u8, Ordering::Release);
    }
    irq_restore(flags);
    yield_now();
    true
}

/// Explicitly wake a blocked task, for event completion or synchronization.
pub fn wake_task(id: TaskId) -> bool {
    let flags = irq_save_disable();
    if !INITIALIZED.load(Ordering::Acquire) || id.0 == 0 || id.0 >= TASK_COUNT.load(Ordering::Acquire) {
        irq_restore(flags);
        return false;
    }
    // SAFETY: Index is validated and interrupts are disabled for the state transition.
    let task = unsafe { task_ptr(id.0) };
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

/// Enable timer-driven preemption after all task contexts are initialized.
pub fn enable_preemption() {
    PREEMPTIVE_ENABLED.store(true, Ordering::Release);
}

/// Number of timer interrupts that switched to a different runnable task.
pub fn preemption_count() -> usize {
    PREEMPTION_COUNT.load(Ordering::Acquire)
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

/// Reset the scheduler after every non-boot task has exited.
pub fn reset_after_all_tasks_exit() -> bool {
    let flags = irq_save_disable();
    if !INITIALIZED.load(Ordering::Acquire) || CURRENT_TASK.load(Ordering::Acquire) != 0 {
        irq_restore(flags);
        return false;
    }
    let count = TASK_COUNT.load(Ordering::Acquire);
    for index in 1..count {
        // SAFETY: Index is bounded by TASK_COUNT and interrupts are disabled.
        let state = unsafe { (*task_ptr(index)).state.load(Ordering::Acquire) };
        if state != TaskState::Exited as u8 {
            irq_restore(flags);
            return false;
        }
    }
    PREEMPTIVE_ENABLED.store(false, Ordering::Release);
    PREEMPTION_COUNT.store(0, Ordering::Release);
    TASK_COUNT.store(1, Ordering::Release);
    CURRENT_TASK.store(0, Ordering::Release);
    INITIALIZED.store(false, Ordering::Release);
    irq_restore(flags);
    true
}

/// Yield voluntarily to the next runnable kernel task.
pub fn yield_now() {
    if INITIALIZED.load(Ordering::Acquire) {
        // SAFETY: IDT vector 0x41 is installed as a ring-0 interrupt gate during boot.
        unsafe { core::arch::asm!("int 0x41", options(nomem, nostack)); }
    }
}

pub fn current_task() -> TaskId {
    TaskId(CURRENT_TASK.load(Ordering::Acquire))
}

pub fn task_state(id: TaskId) -> Option<TaskState> {
    if id.0 >= TASK_COUNT.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: Task table is static and the index is bounded by TASK_COUNT.
    Some(TaskState::from_raw(unsafe { (*task_ptr(id.0)).state.load(Ordering::Acquire) }))
}

pub fn switch_count(id: TaskId) -> Option<usize> {
    if id.0 >= TASK_COUNT.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: Task table is static and the index is bounded by TASK_COUNT.
    Some(unsafe { (*task_ptr(id.0)).switches.load(Ordering::Acquire) })
}

/// Validate synthetic IRETQ frames before running the first task.
pub fn validate_initial_contexts() -> bool {
    let count = TASK_COUNT.load(Ordering::Acquire);
    for index in 1..count {
        // SAFETY: Each saved pointer was built from a live static task stack.
        let rsp = unsafe { (*task_ptr(index)).saved_rsp.load(Ordering::Acquire) };
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

#[cfg(test)]
mod tests {
    use super::*;

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
