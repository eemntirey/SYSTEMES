#![no_std]

//! Minimal, host-independent kernel contract.
//!
//! This crate deliberately contains no Windows, WPF, WebView2, or standard-library
//! dependencies. The UEFI loader will populate BootInfo before transferring control.

pub mod acpi;
pub mod address_space;
pub mod apic;
pub mod console;
pub mod clock;
pub mod heap;
pub mod interrupts;
pub mod memory;
pub mod paging;
pub mod scheduler;
pub mod uaccess;

pub const BOOT_INFO_MAGIC: u64 = 0x5359_5354_454D_4553;
pub const BOOT_INFO_VERSION: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootInfo {
    pub magic: u64,
    pub version: u32,
    pub reserved: u32,
    pub memory_map_address: u64,
    pub memory_map_size: u64,
    pub memory_map_entries: u64,
    pub memory_map_descriptor_size: u64,
    pub memory_map_descriptor_version: u32,
    pub reserved2: u32,
    pub acpi_rsdp_address: u64,
    pub framebuffer_address: u64,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_stride: u32,
    pub framebuffer_pixel_format: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootInfoError {
    InvalidMagic,
    UnsupportedVersion,
    MissingMemoryMap,
    InvalidFramebuffer,
    InvalidMemoryMapDescriptor,
}

impl BootInfo {
    /// Validate the loader-to-kernel handoff without dereferencing physical addresses.
    pub const fn validate(&self) -> Result<(), BootInfoError> {
        if self.magic != BOOT_INFO_MAGIC {
            return Err(BootInfoError::InvalidMagic);
        }
        if self.version != BOOT_INFO_VERSION {
            return Err(BootInfoError::UnsupportedVersion);
        }
        if self.memory_map_address == 0 || self.memory_map_size == 0 || self.memory_map_entries == 0 {
            return Err(BootInfoError::MissingMemoryMap);
        }
        if self.memory_map_descriptor_size == 0
            || self.memory_map_size % self.memory_map_descriptor_size != 0
            || self.memory_map_size / self.memory_map_descriptor_size != self.memory_map_entries
        {
            return Err(BootInfoError::InvalidMemoryMapDescriptor);
        }
        let framebuffer_present = self.framebuffer_address != 0;
        if framebuffer_present
            && (self.framebuffer_width == 0
                || self.framebuffer_height == 0
                || self.framebuffer_stride < self.framebuffer_width)
        {
            return Err(BootInfoError::InvalidFramebuffer);
        }
        Ok(())
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_boot_info() -> BootInfo {
        BootInfo {
            magic: BOOT_INFO_MAGIC,
            version: BOOT_INFO_VERSION,
            reserved: 0,
            memory_map_address: 0x1000,
            memory_map_size: 32 * 48,
            memory_map_entries: 32,
            memory_map_descriptor_size: 48,
            memory_map_descriptor_version: 1,
            reserved2: 0,
            acpi_rsdp_address: 0,
            framebuffer_address: 0x8000,
            framebuffer_width: 1280,
            framebuffer_height: 720,
            framebuffer_stride: 1280,
            framebuffer_pixel_format: 0,
        }
    }

    #[test]
    fn accepts_a_valid_loader_handoff() {
        assert_eq!(valid_boot_info().validate(), Ok(()));
    }

    #[test]
    fn rejects_an_invalid_magic_value() {
        let mut info = valid_boot_info();
        info.magic = 0;
        assert_eq!(info.validate(), Err(BootInfoError::InvalidMagic));
    }

    #[test]
    fn rejects_an_unsupported_contract_version() {
        let mut info = valid_boot_info();
        info.version += 1;
        assert_eq!(info.validate(), Err(BootInfoError::UnsupportedVersion));
    }

    #[test]
    fn rejects_a_missing_memory_map() {
        let mut info = valid_boot_info();
        info.memory_map_entries = 0;
        assert_eq!(info.validate(), Err(BootInfoError::MissingMemoryMap));
    }

    #[test]
    fn rejects_an_inconsistent_memory_map_descriptor_size() {
        let mut info = valid_boot_info();
        info.memory_map_descriptor_size = 0;
        assert_eq!(info.validate(), Err(BootInfoError::InvalidMemoryMapDescriptor));
    }

    #[test]
    fn rejects_an_invalid_framebuffer_geometry() {
        let mut info = valid_boot_info();
        info.framebuffer_stride = info.framebuffer_width - 1;
        assert_eq!(info.validate(), Err(BootInfoError::InvalidFramebuffer));
    }

    #[test]
    fn allows_a_headless_boot_handoff() {
        let mut info = valid_boot_info();
        info.framebuffer_address = 0;
        info.framebuffer_width = 0;
        info.framebuffer_height = 0;
        info.framebuffer_stride = 0;
        assert_eq!(info.validate(), Ok(()));
    }
}
