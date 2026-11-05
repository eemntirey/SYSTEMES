//! Single-core preemptive kernel scheduler for x86_64.
//!
//! Contexts are captured by the Local APIC timer interrupt stub. Every task owns
//! a private ring-0 stack. This is an early kernel scheduler: address spaces are
//! shared until user-mode isolation is implemented in a later kernel phase.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

pub const MAX_TASKS: usize = 8;
const STACK_SIZE: usize = 16 * 1024;
const TIMER_VECTOR: usize = 0x40;
const KERNEL_CODE_SELECTOR: usize = 0x08;
const INITIAL_RFLAGS: usize = 0x202;
const SAVED_REGISTER_COUNT: usize = 15;
const INTERRUPT_FRAME_WORDS: usize = SAVED_REGISTER_COUNT + 5;

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

pub type TaskEntry = extern "C" fn() -> !;

#[repr(align(16))]
struct TaskStack([u8; STACK_SIZE]);

struct Task {
    stack: TaskStack,
    saved_rsp: AtomicUsize,
    state: AtomicU8,
    switches: AtomicUsize,
}

impl Task {
    const fn new() -> Self {
        Self {
            stack: TaskStack([0; STACK_SIZE]),
            saved_rsp: AtomicUsize::new(0),
            state: AtomicU8::new(TaskState::Empty as u8),
            switches: AtomicUsize::new(0),
        }
    }
}

static mut TASKS: [Task; MAX_TASKS] = [const { Task::new() }; MAX_TASKS];
static TASK_COUNT: AtomicUsize = AtomicUsize::new(1);
static CURRENT_TASK: AtomicUsize = AtomicUsize::new(0);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

unsafe fn task_ptr(index: usize) -> *mut Task {
    // SAFETY: Callers validate the index against TASK_COUNT/MAX_TASKS. All task
    // mutation is serialized by single-core interrupt exclusion.
    unsafe { core::ptr::addr_of_mut!(TASKS).cast::<Task>().add(index) }
}

fn build_initial_context(task: *mut Task, entry: TaskEntry) -> usize {
    // SAFETY: The task is exclusively initialized before the scheduler is enabled.
    unsafe {
        let stack = core::ptr::addr_of_mut!((*task).stack.0).cast::<u8>();
        let top = stack.add(STACK_SIZE) as usize;
        let fake_return = (top - 8) as *mut usize;
        fake_return.write(task_returned as usize);
        let frame = (top - 8 - INTERRUPT_FRAME_WORDS * core::mem::size_of::<usize>())
            as *mut usize;
        for index in 0..SAVED_REGISTER_COUNT {
            frame.add(index).write(0);
        }
        frame.add(15).write(TIMER_VECTOR);
        frame.add(16).write(0);
        frame.add(17).write(entry as usize);
        frame.add(18).write(KERNEL_CODE_SELECTOR);
        frame.add(19).write(INITIAL_RFLAGS);
        frame as usize
    }
}

extern "C" fn task_returned() -> ! {
    loop {
        // A task entry must be non-returning. A returned task is treated as fatal.
        unsafe { core::arch::asm!("cli", "hlt", options(nomem, nostack)); }
    }
}

/// Initialize the boot task and up to seven additional kernel tasks.
/// Must be called once, with maskable interrupts disabled.
pub fn initialize(entries: &[TaskEntry]) -> Result<(), &'static str> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Err("scheduler already initialized");
    }
    if entries.len() >= MAX_TASKS {
        return Err("too many kernel tasks");
    }
    // SAFETY: Boot calls this before STI; no task can access the table yet.
    unsafe {
        for index in 0..MAX_TASKS {
            let task = task_ptr(index);
            (*task).saved_rsp.store(0, Ordering::Relaxed);
            (*task).switches.store(0, Ordering::Relaxed);
            (*task).state.store(TaskState::Empty as u8, Ordering::Relaxed);
        }
        let boot = task_ptr(0);
        (*boot).state.store(TaskState::Runnable as u8, Ordering::Release);
        for (offset, entry) in entries.iter().copied().enumerate() {
            let task = task_ptr(offset + 1);
            let rsp = build_initial_context(task, entry);
            (*task).saved_rsp.store(rsp, Ordering::Release);
            (*task).state.store(TaskState::Runnable as u8, Ordering::Release);
        }
    }
    CURRENT_TASK.store(0, Ordering::Release);
    TASK_COUNT.store(entries.len() + 1, Ordering::Release);
    INITIALIZED.store(true, Ordering::Release);
    Ok(())
}

/// Called only by the Local APIC timer interrupt dispatcher.
/// saved_rsp points at the complete register + IRETQ frame built by assembly.
pub fn on_timer(saved_rsp: usize) -> usize {
    if !INITIALIZED.load(Ordering::Acquire) {
        return saved_rsp;
    }
    let count = TASK_COUNT.load(Ordering::Acquire);
    let current = CURRENT_TASK.load(Ordering::Relaxed);
    if current >= count {
        return saved_rsp;
    }

    // SAFETY: The interrupt gate has IF cleared and this is a single-core scheduler.
    unsafe {
        let current_task = task_ptr(current);
        if (*current_task).state.load(Ordering::Acquire) != TaskState::Exited as u8 {
            (*current_task).saved_rsp.store(saved_rsp, Ordering::Release);
            (*current_task).switches.fetch_add(1, Ordering::Relaxed);
        }

        for distance in 1..=count {
            let candidate = (current + distance) % count;
            let next = task_ptr(candidate);
            if (*next).state.load(Ordering::Acquire) == TaskState::Runnable as u8 {
                let next_rsp = (*next).saved_rsp.load(Ordering::Acquire);
                if next_rsp != 0 {
                    CURRENT_TASK.store(candidate, Ordering::Release);
                    return next_rsp;
                }
            }
        }
    }
    saved_rsp
}

pub fn current_task() -> TaskId {
    TaskId(CURRENT_TASK.load(Ordering::Acquire))
}

pub fn task_state(id: TaskId) -> Option<TaskState> {
    if id.0 >= TASK_COUNT.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: The task table is static and initialized before public access.
    Some(TaskState::from_raw(unsafe { (*task_ptr(id.0)).state.load(Ordering::Acquire) }))
}

pub fn switch_count(id: TaskId) -> Option<usize> {
    if id.0 >= TASK_COUNT.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: The task table is static and initialized before public access.
    Some(unsafe { (*task_ptr(id.0)).switches.load(Ordering::Acquire) })
}

/// Block the current task until another task calls wake for its TaskId.
/// Timer interrupts continue to run while this function waits.
pub fn block_current() {
    let id = current_task();
    if id.0 == 0 || id.0 >= TASK_COUNT.load(Ordering::Acquire) {
        return;
    }
    // SAFETY: State transition is atomic with respect to the local timer IRQ.
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
        (*task_ptr(id.0)).state.store(TaskState::Blocked as u8, Ordering::Release);
        core::arch::asm!("sti", options(nomem, nostack));
    }
    while current_task() != id {
        core::hint::spin_loop();
    }
}

/// Wake a blocked task. Returns false for invalid IDs or tasks not blocked.
pub fn wake(id: TaskId) -> bool {
    if id.0 == 0 || id.0 >= TASK_COUNT.load(Ordering::Acquire) {
        return false;
    }
    // SAFETY: Atomic state transition; scheduler observes it on the next tick.
    let state = unsafe { &(*task_ptr(id.0)).state };
    state.compare_exchange(
        TaskState::Blocked as u8,
        TaskState::Runnable as u8,
        Ordering::AcqRel,
        Ordering::Acquire,
    ).is_ok()
}

/// A single-core IRQ-safe spin mutex. Interrupts are disabled while held so a
/// preempted owner cannot deadlock another task spinning on the same CPU.
pub struct SpinMutex<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SpinMutex<T> {}

impl<T> SpinMutex<T> {
    pub const fn new(value: T) -> Self {
        Self { locked: AtomicBool::new(false), value: UnsafeCell::new(value) }
    }

    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        #[cfg(not(test))]
        let flags: usize = {
            let value: usize;
            // SAFETY: RFLAGS is read and maskable interrupts are disabled at CPL0.
            unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) value, options(nomem)); }
            value
        };
        #[cfg(test)]
        let flags: usize = 0;
        while self.locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        SpinMutexGuard { mutex: self, restore_interrupts: flags & (1 << 9) != 0 }
    }
}

pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
    restore_interrupts: bool,
}

impl<T> Deref for SpinMutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: The guard owns the mutex until Drop.
        unsafe { &*self.mutex.value.get() }
    }
}

impl<T> DerefMut for SpinMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: The guard owns the mutex until Drop.
        unsafe { &mut *self.mutex.value.get() }
    }
}

impl<T> Drop for SpinMutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Release);
        #[cfg(not(test))]
        if self.restore_interrupts {
            // SAFETY: Restore the prior interrupt-enable state after unlock.
            unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
        }
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
    fn spin_mutex_serializes_mutation() {
        let mutex = SpinMutex::new(0usize);
        *mutex.lock() += 1;
        assert_eq!(*mutex.lock(), 1);
    }
}
