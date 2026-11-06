//! Multiprocessor topology and per-CPU startup state.
//!
//! This module is the software foundation for K2.11. It deliberately does not
//! claim that an AP is running merely because firmware listed it: APs move to
//! `Online` only after the future trampoline/entry path completes its handshake.
//! The actual INIT/SIPI trampoline and per-CPU descriptor-table installation are
//! separate steps and must be wired before SMP is enabled.

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::acpi::MadtInfo;

pub const MAX_CPUS: usize = 64;

const STATE_ABSENT: u8 = 0;
const STATE_DISCOVERED: u8 = 1;
const STATE_STARTING: u8 = 2;
const STATE_ONLINE: u8 = 3;
const STATE_FAILED: u8 = 4;

/// Lifecycle state of one logical processor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CpuState {
    Absent = STATE_ABSENT,
    Discovered = STATE_DISCOVERED,
    Starting = STATE_STARTING,
    Online = STATE_ONLINE,
    Failed = STATE_FAILED,
}

impl CpuState {
    fn from_raw(value: u8) -> Self {
        match value {
            STATE_DISCOVERED => Self::Discovered,
            STATE_STARTING => Self::Starting,
            STATE_ONLINE => Self::Online,
            STATE_FAILED => Self::Failed,
            _ => Self::Absent,
        }
    }
}

/// Firmware-provided identity for a processor which ACPI marks as enabled.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpuDescriptor {
    pub processor_uid: u8,
    pub apic_id: u8,
}

/// A bounded, validated view of the enabled processors in the MADT.
///
/// The BSP is always placed at index zero. Remaining processors retain their
/// firmware order, making indices stable for a given MADT.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuTopology {
    cpus: [CpuDescriptor; MAX_CPUS],
    len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyError {
    EmptyMadt,
    TooManyCpus,
    MissingBootstrapProcessor,
    DuplicateApicId,
}

impl CpuTopology {
    /// Build the boot-time CPU inventory from enabled MADT Local APIC records.
    ///
    /// Entries with only the ACPI "online capable" bit set are not started:
    /// hot-plug support is not implemented yet.
    pub fn from_madt(madt: &MadtInfo, bsp_apic_id: u8) -> Result<Self, TopologyError> {
        if madt.local_apic_count == 0 {
            return Err(TopologyError::EmptyMadt);
        }

        let mut topology = Self {
            cpus: [CpuDescriptor::default(); MAX_CPUS],
            len: 0,
        };
        let mut bsp = None;

        for entry in madt.local_apics.iter().take(madt.local_apic_count) {
            // MADT Local APIC flags bit 0 means enabled. Bit 1 alone means
            // online-capable and is intentionally excluded until hot-plug exists.
            if entry.flags & 1 == 0 {
                continue;
            }

            if topology.cpus[..topology.len]
                .iter()
                .any(|cpu| cpu.apic_id == entry.apic_id)
            {
                return Err(TopologyError::DuplicateApicId);
            }
            if topology.len == MAX_CPUS {
                return Err(TopologyError::TooManyCpus);
            }

            let descriptor = CpuDescriptor {
                processor_uid: entry.processor_uid,
                apic_id: entry.apic_id,
            };
            if entry.apic_id == bsp_apic_id {
                bsp = Some(descriptor);
            } else {
                topology.cpus[topology.len] = descriptor;
                topology.len += 1;
            }
        }

        let bsp = bsp.ok_or(TopologyError::MissingBootstrapProcessor)?;
        if topology.len == MAX_CPUS {
            return Err(TopologyError::TooManyCpus);
        }
        for index in (1..=topology.len).rev() {
            topology.cpus[index] = topology.cpus[index - 1];
        }
        topology.cpus[0] = bsp;
        topology.len += 1;

        Ok(topology)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn bsp(&self) -> CpuDescriptor {
        self.cpus[0]
    }

    pub fn get(&self, index: usize) -> Option<CpuDescriptor> {
        if index < self.len {
            Some(self.cpus[index])
        } else {
            None
        }
    }

    pub fn application_processors(&self) -> &[CpuDescriptor] {
        &self.cpus[1..self.len]
    }
}

struct CpuSlot {
    apic_id: AtomicU8,
    state: AtomicU8,
}

impl CpuSlot {
    const fn new() -> Self {
        Self {
            apic_id: AtomicU8::new(0),
            state: AtomicU8::new(STATE_ABSENT),
        }
    }
}

/// Atomic per-CPU lifecycle registry shared by the BSP and AP entry paths.
pub struct CpuRegistry {
    initialized: AtomicBool,
    slots: [CpuSlot; MAX_CPUS],
    count: AtomicU8,
}

impl CpuRegistry {
    pub const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            slots: [const { CpuSlot::new() }; MAX_CPUS],
            count: AtomicU8::new(0),
        }
    }

    /// Publish the validated topology. The BSP is marked online; APs remain
    /// discovered until their startup handshake succeeds.
    pub fn initialize(&self, topology: &CpuTopology) -> Result<(), RegistryError> {
        if self.initialized.load(Ordering::Acquire) {
            return Err(RegistryError::AlreadyInitialized);
        }
        if topology.is_empty() || topology.len() > MAX_CPUS {
            return Err(RegistryError::InvalidTopology);
        }

        for index in 0..topology.len() {
            let descriptor = topology.get(index).ok_or(RegistryError::InvalidTopology)?;
            self.slots[index].apic_id.store(descriptor.apic_id, Ordering::Relaxed);
            self.slots[index].state.store(
                if index == 0 { STATE_ONLINE } else { STATE_DISCOVERED },
                Ordering::Relaxed,
            );
        }
        self.count.store(topology.len() as u8, Ordering::Relaxed);
        self.initialized.store(true, Ordering::Release);
        Ok(())
    }

    pub fn count(&self) -> usize {
        if !self.initialized.load(Ordering::Acquire) {
            0
        } else {
            self.count.load(Ordering::Acquire) as usize
        }
    }

    pub fn apic_id(&self, index: usize) -> Option<u8> {
        if index >= self.count() {
            return None;
        }
        Some(self.slots[index].apic_id.load(Ordering::Acquire))
    }

    pub fn state(&self, index: usize) -> CpuState {
        if index >= self.count() {
            return CpuState::Absent;
        }
        CpuState::from_raw(self.slots[index].state.load(Ordering::Acquire))
    }

    /// Claim an AP for startup. The BSP (index zero) cannot enter this path.
    pub fn begin_startup(&self, index: usize) -> Result<(), RegistryError> {
        if index == 0 || index >= self.count() {
            return Err(RegistryError::InvalidCpuIndex);
        }
        self.slots[index]
            .state
            .compare_exchange(
                STATE_DISCOVERED,
                STATE_STARTING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|_| ())
            .map_err(|_| RegistryError::InvalidStateTransition)
    }

    /// Complete the AP handshake after its stack, GDT/IDT and LAPIC are ready.
    pub fn mark_online(&self, index: usize) -> Result<(), RegistryError> {
        self.transition_ap(index, STATE_STARTING, STATE_ONLINE)
    }

    pub fn mark_failed(&self, index: usize) -> Result<(), RegistryError> {
        self.transition_ap(index, STATE_STARTING, STATE_FAILED)
    }

    fn transition_ap(&self, index: usize, from: u8, to: u8) -> Result<(), RegistryError> {
        if index == 0 || index >= self.count() {
            return Err(RegistryError::InvalidCpuIndex);
        }
        self.slots[index]
            .state
            .compare_exchange(from, to, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| RegistryError::InvalidStateTransition)
    }

    pub fn online_count(&self) -> usize {
        (0..self.count())
            .filter(|index| self.state(*index) == CpuState::Online)
            .count()
    }
}

impl Default for CpuRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryError {
    AlreadyInitialized,
    InvalidTopology,
    InvalidCpuIndex,
    InvalidStateTransition,
}

/// Global registry used by early boot and subsequent per-CPU initialization.
pub static CPU_REGISTRY: CpuRegistry = CpuRegistry::new();

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acpi::{LocalApic, MAX_LOCAL_APICS};

    fn madt(entries: &[(u8, u8, u32)]) -> MadtInfo {
        let mut info = MadtInfo {
            local_apic_address: 0xFEE0_0000,
            flags: 1,
            local_apics: [LocalApic::default(); MAX_LOCAL_APICS],
            local_apic_count: entries.len(),
            io_apics: [Default::default(); crate::acpi::MAX_IO_APICS],
            io_apic_count: 0,
            overrides: [Default::default(); crate::acpi::MAX_ISO],
            override_count: 0,
        };
        for (index, (uid, apic_id, flags)) in entries.iter().copied().enumerate() {
            info.local_apics[index] = LocalApic {
                processor_uid: uid,
                apic_id,
                flags,
            };
        }
        info
    }

    #[test]
    fn topology_places_bsp_first_and_excludes_disabled_processors() {
        let info = madt(&[(0, 2, 1), (1, 7, 1), (2, 9, 0), (3, 11, 2)]);
        let topology = CpuTopology::from_madt(&info, 7).unwrap();
        assert_eq!(topology.len(), 2);
        assert_eq!(topology.bsp().apic_id, 7);
        assert_eq!(topology.get(1).unwrap().apic_id, 2);
    }

    #[test]
    fn topology_rejects_missing_bsp_and_duplicate_apic_ids() {
        let missing = madt(&[(0, 2, 1)]);
        assert_eq!(
            CpuTopology::from_madt(&missing, 7),
            Err(TopologyError::MissingBootstrapProcessor)
        );

        let duplicate = madt(&[(0, 7, 1), (1, 7, 1)]);
        assert_eq!(
            CpuTopology::from_madt(&duplicate, 7),
            Err(TopologyError::DuplicateApicId)
        );
    }

    #[test]
    fn registry_requires_a_valid_ap_startup_transition() {
        let info = madt(&[(0, 2, 1), (1, 7, 1)]);
        let topology = CpuTopology::from_madt(&info, 2).unwrap();
        let registry = CpuRegistry::new();
        registry.initialize(&topology).unwrap();

        assert_eq!(registry.online_count(), 1);
        assert_eq!(registry.state(1), CpuState::Discovered);
        assert_eq!(registry.mark_online(1), Err(RegistryError::InvalidStateTransition));
        registry.begin_startup(1).unwrap();
        assert_eq!(registry.state(1), CpuState::Starting);
        registry.mark_online(1).unwrap();
        assert_eq!(registry.online_count(), 2);
        assert_eq!(registry.begin_startup(1), Err(RegistryError::InvalidStateTransition));
    }
}
