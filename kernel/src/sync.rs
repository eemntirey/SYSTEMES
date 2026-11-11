//! Minimal acquire/release spin lock used by SMP bring-up and shared kernel state.
//!
//! Callers that may be interrupted while holding a lock must disable local
//! interrupts first. Scheduler run-queue locks use a dedicated CPU-index order
//! and are never acquired recursively.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

// SAFETY: every access to `data` goes through a guard that holds `locked`.
unsafe impl<T: Send> Sync for SpinLock<T> {}

pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SpinLockGuard { lock: self }
    }

    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| SpinLockGuard { lock: self })
    }

    /// Exclusive access through `&mut self` needs no atomic operation.
    pub fn get_mut(&mut self) -> &mut T {
        // SAFETY: `&mut self` guarantees no other reference to `data` exists.
        unsafe { &mut *self.data.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: the guard holds the lock, so no other access is in flight.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the guard holds the lock, so no other access is in flight.
        unsafe { &mut *self.lock.data.get() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_grants_exclusive_mutable_access() {
        let lock = SpinLock::new(1u32);
        {
            let mut guard = lock.lock();
            *guard += 1;
        }
        assert_eq!(*lock.lock(), 2);
    }

    #[test]
    fn try_lock_fails_while_the_guard_is_alive() {
        let lock = SpinLock::new(0u32);
        let guard = lock.lock();
        assert!(lock.try_lock().is_none());
        drop(guard);
        assert!(lock.try_lock().is_some());
    }

    #[test]
    fn get_mut_does_not_require_the_lock_bit() {
        let mut lock = SpinLock::new(7u32);
        *lock.get_mut() = 9;
        assert_eq!(*lock.lock(), 9);
    }
}
