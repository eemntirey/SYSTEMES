//! Cooperative single-core kernel task scheduler for x86_64.
//!
//! Tasks share the kernel address space and switch only at explicit yield points.
//! Preemptive scheduling is a separate follow-up capability.

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

pub const MAX_TASKS: usize = 8;
const STACK_SIZE: usize = 16 * 1024;
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

#[repr(align(16))]
struct TaskStack([u8; STACK_SIZE]);

struct Task {
    stack: TaskStack,
    saved_rsp: AtomicUsize,
    state: AtomicU8,
    switches: AtomicUsize,
    entry: AtomicUsize,
}

impl Task {
    const fn new() -> Self {
        Self {
            stack: TaskStack([0; STACK_SIZE]),
            saved_rsp: AtomicUsize::new(0),
            state: AtomicU8::new(TaskState::Empty as u8),
            switches: AtomicUsize::new(0),
            entry: AtomicUsize::new(0),
        }
    }
}

static mut TASKS: [Task; MAX_TASKS] = [const { Task::new() }; MAX_TASKS];
static TASK_COUNT: AtomicUsize = AtomicUsize::new(1);
static CURRENT_TASK: AtomicUsize = AtomicUsize::new(0);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

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
        fake_return.write(task_returned as usize);
        let frame = (top - 8 - INTERRUPT_FRAME_WORDS * core::mem::size_of::<usize>()) as *mut usize;
        for index in 0..SAVED_REGISTER_COUNT {
            frame.add(index).write(0);
        }
        frame.add(15).write(YIELD_VECTOR);
        frame.add(16).write(0);
        frame.add(17).write(task_bootstrap as usize);
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
    // SAFETY: Called during single-core boot before any task can access the table.
    unsafe {
        for index in 0..MAX_TASKS {
            let task = task_ptr(index);
            (*task).saved_rsp.store(0, Ordering::Relaxed);
            (*task).switches.store(0, Ordering::Relaxed);
            (*task).entry.store(0, Ordering::Relaxed);
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

/// Called by the software-yield interrupt gate. Returns the stack to restore.
pub fn on_yield(saved_rsp: usize) -> usize {
    if !INITIALIZED.load(Ordering::Acquire) {
        return saved_rsp;
    }
    let count = TASK_COUNT.load(Ordering::Acquire);
    let current = CURRENT_TASK.load(Ordering::Relaxed);
    if current >= count {
        return saved_rsp;
    }

    // SAFETY: The yield interrupt gate clears IF; this scheduler is single-core.
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
                CURRENT_TASK.store(next, Ordering::Release);
                return next_rsp;
            }
        }
    }
    saved_rsp
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
                && frame.add(17).read() == task_bootstrap as usize
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
        current_task() == TaskId(0)
            && COUNTS.iter().all(|count| count.load(Ordering::Acquire) == 5)
            && (1..=3).all(|index| switch_count(TaskId(index)).unwrap_or(0) > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
