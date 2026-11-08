//! PCI configuration-space discovery for the transitional virtio-blk transport.

const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

const VIRTIO_VENDOR_ID: u16 = 0x1AF4;
const VIRTIO_BLK_LEGACY_ID: u16 = 0x1001;
const VIRTIO_BLK_MODERN_ID: u16 = 0x1042;
const PCI_COMMAND: u8 = 0x04;
const PCI_BAR0: u8 = 0x10;
const PCI_IO_SPACE: u16 = 1;
const PCI_BUS_MASTER: u16 = 1 << 2;
const LEGACY_MAX_REGISTER: u16 = 0x34;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciError {
    DeviceNotFound,
    NotVirtioBlk,
    InvalidBar,
    UnsupportedTransport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub bar0: u64,
}

fn config_address(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((function as u32) << 8)
        | (offset as u32 & 0xFC)
}

fn legacy_io_base(bar: u32) -> Result<u16, PciError> {
    if bar & 1 == 0 || bar & !3 > u16::MAX as u32 {
        return Err(PciError::InvalidBar);
    }
    let base = (bar & !3) as u16;
    if base == 0 || base.checked_add(LEGACY_MAX_REGISTER).is_none() {
        return Err(PciError::InvalidBar);
    }
    Ok(base)
}

/// Read a 32-bit value from PCI configuration space.
///
/// # Safety
///
/// Standard PCI configuration I/O ports must be available to ring 0.
pub unsafe fn pci_read_dword(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    let address = config_address(bus, slot, function, offset);
    // SAFETY: The caller guarantees access to the PCI configuration ports.
    unsafe {
        crate::console::outl(PCI_CONFIG_ADDRESS, address);
        crate::console::inl(PCI_CONFIG_DATA)
    }
}

/// Write a 16-bit PCI configuration field without disturbing adjacent status bits.
///
/// # Safety
///
/// Standard PCI configuration I/O ports must be available to ring 0.
pub unsafe fn pci_write_word(bus: u8, slot: u8, function: u8, offset: u8, value: u16) {
    let address = config_address(bus, slot, function, offset);
    // SAFETY: The caller guarantees access to the PCI configuration ports.
    unsafe {
        crate::console::outl(PCI_CONFIG_ADDRESS, address);
        crate::console::outw(PCI_CONFIG_DATA + (offset & 2) as u16, value);
    }
}

/// Locate virtio-blk across all PCI buses, slots, and functions.
///
/// Only the transitional I/O-port transport is supported. Modern memory-BAR
/// devices are reported explicitly rather than misconfigured as port devices.
///
/// # Safety
///
/// Standard PCI configuration I/O ports must be available to ring 0.
pub unsafe fn discover_virtio_blk() -> Result<PciDevice, PciError> {
    let mut found_other_virtio_device = false;
    let mut visited_buses = [false; 256];
    let mut bus_queue = [0u8; 256];
    let mut head = 0usize;
    let mut tail = 1usize;
    bus_queue[0] = 0;
    visited_buses[0] = true;

    while head < tail {
        let bus = bus_queue[head];
        head += 1;
        for slot in 0..32u8 {
            let function_zero_ids = unsafe { pci_read_dword(bus, slot, 0, 0) };
            if function_zero_ids as u16 == u16::MAX {
                continue;
            }
            let header_type =
                (unsafe { pci_read_dword(bus, slot, 0, 0x0C) } >> 16) as u8;
            let function_count = if header_type & 0x80 != 0 { 8 } else { 1 };
            for function in 0..function_count {
                let ids = if function == 0 {
                    function_zero_ids
                } else {
                    unsafe { pci_read_dword(bus, slot, function, 0) }
                };
                let vendor_id = ids as u16;
                if vendor_id == u16::MAX {
                    continue;
                }
                let device_id = (ids >> 16) as u16;
                let class_and_revision =
                    unsafe { pci_read_dword(bus, slot, function, 0x08) };
                let class = (class_and_revision >> 24) as u8;
                let subclass = (class_and_revision >> 16) as u8;
                if class == 0x06 && subclass == 0x04 {
                    let buses = unsafe { pci_read_dword(bus, slot, function, 0x18) };
                    let secondary = (buses >> 8) as u8;
                    let subordinate = (buses >> 16) as u8;
                    if secondary != 0
                        && secondary <= subordinate
                        && !visited_buses[secondary as usize]
                    {
                        visited_buses[secondary as usize] = true;
                        if tail < bus_queue.len() {
                            bus_queue[tail] = secondary;
                            tail += 1;
                        }
                    }
                }
                if vendor_id != VIRTIO_VENDOR_ID {
                    continue;
                }
                if device_id == VIRTIO_BLK_MODERN_ID {
                    return Err(PciError::UnsupportedTransport);
                }
                if device_id != VIRTIO_BLK_LEGACY_ID {
                    found_other_virtio_device = true;
                    continue;
                }

                let bar0 = unsafe { pci_read_dword(bus, slot, function, PCI_BAR0) };
                let io_base = legacy_io_base(bar0)?;
                return Ok(PciDevice {
                    bus,
                    slot,
                    function,
                    vendor_id,
                    device_id,
                    bar0: io_base as u64,
                });
            }
        }
    }
    if found_other_virtio_device {
        Err(PciError::NotVirtioBlk)
    } else {
        Err(PciError::DeviceNotFound)
    }
}

/// Enable I/O decoding and PCI bus mastering for a discovered device.
///
/// # Safety
///
/// `device` must identify the same PCI function returned by discovery.
pub unsafe fn enable_io_and_bus_master(device: PciDevice) {
    let command = unsafe {
        pci_read_dword(device.bus, device.slot, device.function, PCI_COMMAND) as u16
    };
    unsafe {
        pci_write_word(
            device.bus,
            device.slot,
            device.function,
            PCI_COMMAND,
            command | PCI_IO_SPACE | PCI_BUS_MASTER,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_address_encodes_bus_device_function_and_register() {
        assert_eq!(
            config_address(2, 3, 4, 0x12),
            0x8000_0000 | (2 << 16) | (3 << 11) | (4 << 8) | 0x10
        );
    }

    #[test]
    fn accepts_only_valid_legacy_io_bars() {
        assert_eq!(legacy_io_base(0xC001), Ok(0xC000));
        assert_eq!(legacy_io_base(0xC000), Err(PciError::InvalidBar));
        assert_eq!(legacy_io_base(1), Err(PciError::InvalidBar));
    }
}
