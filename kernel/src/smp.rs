//! SMP topology and per-CPU lifecycle primitives for x86_64.
//!
//! This is the SMP data-plane foundation only. It does not send INIT/SIPI IPIs
//! or claim secondary CPUs are running; AP trampoline and per-CPU descriptor
//! initialization remain explicit follow-up work.

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicUsize, Ordering};

pub const MAX_CPUS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CpuState {
    Empty = 0,
    Discovered = 1,
    Prepared = 2,
    Starting = 3,
    Online = 4,
    Failed = 5,
}

impl CpuState {
    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Discovered,
            2 => Self::Prepared,
            3 => Self::Starting,
            4 => Self::Online,
            5 => Self::Failed,
            _ => Self::Empty,
        }
    }
}

#[repr(C, align(64))]
pub struct CpuLocal {
    apic_id: AtomicU32,
    logical_id: AtomicU32,
    state: AtomicU8,
    startup_error: AtomicU32,
}

impl CpuLocal {
    const fn new() -> Self {
        Self {
            apic_id: AtomicU32::new(u32::MAX),
            logical_id: AtomicU32::new(u32::MAX),
            state: AtomicU8::new(CpuState::Empty as u8),
            startup_error: AtomicU32::new(0),
        }
    }

    pub fn apic_id(&self) -> Option<u32> {
        let id = self.apic_id.load(Ordering::Acquire);
        (id != u32::MAX).then_some(id)
    }

    pub fn logical_id(&self) -> Option<usize> {
        let id = self.logical_id.load(Ordering::Acquire);
        (id != u32::MAX).then_some(id as usize)
    }

    pub fn state(&self) -> CpuState {
        CpuState::from_raw(self.state.load(Ordering::Acquire))
    }

    pub fn startup_error(&self) -> u32 {
        self.startup_error.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmpError {
    CapacityExceeded,
    DuplicateApicId,
    BootstrapAlreadyRegistered,
    InvalidLogicalId,
    InvalidStateTransition,
}

pub struct CpuRegistry {
    cpus: [CpuLocal; MAX_CPUS],
    count: AtomicUsize,
    registration_lock: AtomicBool,
    bootstrap_logical_id: AtomicUsize,
}

impl CpuRegistry {
    pub const fn new() -> Self {
        Self {
            cpus: [const { CpuLocal::new() }; MAX_CPUS],
            count: AtomicUsize::new(0),
            registration_lock: AtomicBool::new(false),
            bootstrap_logical_id: AtomicUsize::new(usize::MAX),
        }
    }

    fn lock_registration(&self) -> RegistrationGuard<'_> {
        while self.registration_lock.compare_exchange(
            false, true, Ordering::Acquire, Ordering::Relaxed
        ).is_err() {
            core::hint::spin_loop();
        }
        RegistrationGuard(&self.registration_lock)
    }

    /// Register one firmware-discovered processor. The BSP starts Online;
    /// application processors start Discovered and require explicit transitions.
    pub fn register(&self, apic_id: u32, bootstrap: bool) -> Result<usize, SmpError> {
        let _guard = self.lock_registration();
        let count = self.count.load(Ordering::Relaxed);

        if bootstrap && self.bootstrap_logical_id.load(Ordering::Relaxed) != usize::MAX {
            return Err(SmpError::BootstrapAlreadyRegistered);
        }
        if self.cpus.iter().take(count).any(|cpu| {
            cpu.apic_id.load(Ordering::Relaxed) == apic_id
        }) {
            return Err(SmpError::DuplicateApicId);
        }
        if count == MAX_CPUS {
            return Err(SmpError::CapacityExceeded);
        }

        let cpu = &self.cpus[count];
        cpu.apic_id.store(apic_id, Ordering::Relaxed);
        cpu.logical_id.store(count as u32, Ordering::Relaxed);
        cpu.startup_error.store(0, Ordering::Relaxed);
        let initial = if bootstrap { CpuState::Online } else { CpuState::Discovered };
        cpu.state.store(initial as u8, Ordering::Release);
        self.count.store(count + 1, Ordering::Release);
        if bootstrap {
            self.bootstrap_logical_id.store(count, Ordering::Release);
        }
        Ok(count)
    }

    pub fn count(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }

    pub fn bootstrap_logical_id(&self) -> Option<usize> {
        let id = self.bootstrap_logical_id.load(Ordering::Acquire);
        (id != usize::MAX).then_some(id)
    }

    pub fn cpu(&self, logical_id: usize) -> Option<&CpuLocal> {
        if logical_id >= self.count() { return None; }
        self.cpus.get(logical_id)
    }

    pub fn find_by_apic_id(&self, apic_id: u32) -> Option<usize> {
        self.cpus.iter().take(self.count()).position(|cpu| {
            cpu.apic_id.load(Ordering::Acquire) == apic_id
        })
    }

    pub fn mark_prepared(&self, id: usize) -> Result<(), SmpError> {
        self.transition(id, CpuState::Discovered, CpuState::Prepared)
    }

    pub fn begin_startup(&self, id: usize) -> Result<(), SmpError> {
        self.transition(id, CpuState::Prepared, CpuState::Starting)
    }

    pub fn mark_online(&self, id: usize) -> Result<(), SmpError> {
        self.transition(id, CpuState::Starting, CpuState::Online)
    }

    pub fn mark_failed(&self, id: usize, error: u32) -> Result<(), SmpError> {
        let cpu = self.cpu(id).ok_or(SmpError::InvalidLogicalId)?;
        let state = cpu.state.load(Ordering::Acquire);
        if state != CpuState::Discovered as u8
            && state != CpuState::Prepared as u8
            && state != CpuState::Starting as u8 {
            return Err(SmpError::InvalidStateTransition);
        }
        cpu.startup_error.store(error, Ordering::Relaxed);
        cpu.state.compare_exchange(
            state, CpuState::Failed as u8, Ordering::AcqRel, Ordering::Acquire
        ).map(|_| ()).map_err(|_| SmpError::InvalidStateTransition)
    }

    fn transition(&self, id: usize, from: CpuState, to: CpuState) -> Result<(), SmpError> {
        let cpu = self.cpu(id).ok_or(SmpError::InvalidLogicalId)?;
        cpu.state.compare_exchange(
            from as u8, to as u8, Ordering::AcqRel, Ordering::Acquire
        ).map(|_| ()).map_err(|_| SmpError::InvalidStateTransition)
    }
}

impl Default for CpuRegistry {
    fn default() -> Self { Self::new() }
}

struct RegistrationGuard<'a>(&'a AtomicBool);

impl Drop for RegistrationGuard<'_> {
    fn drop(&mut self) { self.0.store(false, Ordering::Release); }
}

pub static CPU_REGISTRY: CpuRegistry = CpuRegistry::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_bootstrap_and_secondary_processors() {
        let registry = CpuRegistry::new();
        assert_eq!(registry.register(0, true), Ok(0));
        assert_eq!(registry.register(2, false), Ok(1));
        assert_eq!(registry.count(), 2);
        assert_eq!(registry.bootstrap_logical_id(), Some(0));
        assert_eq!(registry.find_by_apic_id(2), Some(1));
        assert_eq!(registry.cpu(0).unwrap().state(), CpuState::Online);
        assert_eq!(registry.cpu(1).unwrap().state(), CpuState::Discovered);
    }

    #[test]
    fn rejects_duplicate_apic_ids_and_second_bootstrap() {
        let registry = CpuRegistry::new();
        assert_eq!(registry.register(7, true), Ok(0));
        assert_eq!(registry.register(7, false), Err(SmpError::DuplicateApicId));
        assert_eq!(registry.register(8, true), Err(SmpError::BootstrapAlreadyRegistered));
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn secondary_must_follow_the_startup_state_machine() {
        let registry = CpuRegistry::new();
        registry.register(0, true).unwrap();
        let ap = registry.register(3, false).unwrap();
        assert_eq!(registry.mark_online(ap), Err(SmpError::InvalidStateTransition));
        registry.mark_prepared(ap).unwrap();
        registry.begin_startup(ap).unwrap();
        registry.mark_online(ap).unwrap();
        assert_eq!(registry.cpu(ap).unwrap().state(), CpuState::Online);
        assert_eq!(registry.mark_online(ap), Err(SmpError::InvalidStateTransition));
    }

    #[test]
    fn records_startup_failure() {
        let registry = CpuRegistry::new();
        registry.register(0, true).unwrap();
        let ap = registry.register(1, false).unwrap();
        registry.mark_failed(ap, 0x10).unwrap();
        assert_eq!(registry.cpu(ap).unwrap().state(), CpuState::Failed);
        assert_eq!(registry.cpu(ap).unwrap().startup_error(), 0x10);
    }

    #[test]
    fn rejects_invalid_logical_ids() {
        let registry = CpuRegistry::new();
        assert_eq!(registry.mark_prepared(0), Err(SmpError::InvalidLogicalId));
        assert_eq!(registry.cpu(MAX_CPUS), None);
    }

    #[test]
    fn enforces_fixed_registry_capacity() {
        let registry = CpuRegistry::new();
        for id in 0..MAX_CPUS as u32 {
            registry.register(id, id == 0).unwrap();
        }
        assert_eq!(registry.register(1000, false), Err(SmpError::CapacityExceeded));
    }
}
