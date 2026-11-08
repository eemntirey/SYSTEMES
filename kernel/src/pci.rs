//! Minimal PCI enumeration for virtio-blk discovery.
//!
//! Reads the PCI configuration space via I/O ports 0xCF8/0xCFC. Only the
//! first virtio-blk function (vendor 0x1AF4, device 0x1001/0x1043) is
//! selected. No PCI bus enumeration beyond device 0/function 0 is performed:
//! this is sufficient for QEMU, which places virtio-blk at bus 0, slot 0.

const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

const PCI_VENDOR_ID: u16 = 0x1AF4; // virtio
const PCI_DEVICE_ID_BLK_MODERN: u16 = 0x1043;
const PCI_DEVICE_ID_BLK_LEGACY: u16 = 0x1001;

const PCI_REG_VENDOR_ID: u8 = 0x00;
const PCI_REG_DEVICE_ID: u8 = 0x02;
const PCI_REG_BAR0: u8 = 0x10;
const PCI_REG_INTERRUPT_LINE: u8 = 0x3C;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciError {
    DeviceNotFound,
    NotVirtioBlk,
    InvalidBar,
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

/// Read a 32-bit value from the PCI configuration space.
///
/// # Safety
///
/// PCI configuration space access via I/O ports is only valid when the PCI
/// host bridge is configured for standard I/O port configuration access.
pub unsafe fn pci_read_dword(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    let address: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((function as u32) << 8)
        | (offset as u32 & 0xFC);
    // SAFETY: PCI configuration space I/O ports.
    unsafe {
        crate::console::outb(PCI_CONFIG_ADDRESS, (address & 0xFF) as u8);
        crate::console::outb(PCI_CONFIG_ADDRESS + 1, (address >> 8) as u8);
        crate::console::outb(PCI_CONFIG_ADDRESS + 2, (address >> 16) as u8);
        crate::console::outb(PCI_CONFIG_ADDRESS + 3, (address >> 24) as u8);
    }
    let value: u32;
    // SAFETY: PCI configuration space data port.
    unsafe {
        core::arch::asm!(
            "in eax, dx",
            in("dx") PCI_CONFIG_DATA,
            out("eax") value,
            options(nomem, nostack, preserves_flags)
        )
    };
    value
}

/// Discover the first virtio-blk device on PCI bus 0, slot 0, function 0.
///
/// Returns the device descriptor with its BAR0 base address, or an error if
/// no virtio-blk device is present.
///
/// # Safety
///
/// PCI configuration space access is only valid in ring 0 with the PCI host
/// bridge in standard I/O port configuration mode.
pub unsafe fn discover_virtio_blk() -> Result<PciDevice, PciError> {
    let vendor_id = pci_read_dword(0, 0, 0, PCI_REG_VENDOR_ID) as u16;
    if vendor_id == 0xFFFF {
        return Err(PciError::DeviceNotFound);
    }
    let device_id = (pci_read_dword(0, 0, 0, PCI_REG_DEVICE_ID) as u16);
    if vendor_id != PCI_VENDOR_ID
        || (device_id != PCI_DEVICE_ID_BLK_MODERN && device_id != PCI_DEVICE_ID_BLK_LEGACY)
    {
        return Err(PciError::NotVirtioBlk);
    }

    let bar0_raw = pci_read_dword(0, 0, 0, PCI_REG_BAR0);
    // BAR0 is an I/O port base (bit 0 = 1) for virtio-blk on QEMU.
    if bar0_raw & 1 == 0 {
        return Err(PciError::InvalidBar);
    }
    let bar0 = (bar0_raw & !0x3) as u64;

    Ok(PciDevice {
        bus: 0,
        slot: 0,
        function: 0,
        vendor_id,
        device_id,
        bar0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests validate pure constants and layout, not hardware access.
    #[test]
    fn pci_constants() {
        assert_eq!(PCI_CONFIG_ADDRESS, 0xCF8);
        assert_eq!(PCI_CONFIG_DATA, 0xCFC);
        assert_eq!(PCI_VENDOR_ID, 0x1AF4);
        assert_eq!(PCI_DEVICE_ID_BLK_MODERN, 0x1043);
        assert_eq!(PCI_DEVICE_ID_BLK_LEGACY, 0x1001);
    }
}
