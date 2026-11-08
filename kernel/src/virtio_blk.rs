//! Minimal virtio-blk block device driver.
//!
//! Discovers the virtio-blk device through PCI, sets up a single virtqueue
//! of fixed size, and issues block read requests. Only the 512-byte sector
//! read path is implemented. All memory used by the virtqueue (descriptor
//! table, available ring, used ring) is allocated from the physical frame
//! allocator and exposed to the device through physical addresses.

use crate::memory::{PhysicalFrameAllocator, PAGE_SIZE};

const VIRTIO_BLK_SECTOR_SIZE: u64 = 512;
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;
const VIRTIO_BLK_S_OK: u8 = 0;
const VIRTIO_BLK_S_IOERR: u8 = 1;

/// Size of the single virtqueue (number of descriptors).
const QUEUE_SIZE: u16 = 8;

/// I/O port offsets relative to the virtio-blk BAR0 base.
mod reg {
    pub const DEVICE_FEATURES: u16 = 0x00;
    pub const DEVICE_FEATURES_SEL: u16 = 0x04;
    pub const DRIVER_FEATURES: u16 = 0x10;
    pub const DRIVER_FEATURES_SEL: u16 = 0x14;
    pub const QUEUE_SEL: u16 = 0x34;
    pub const QUEUE_NUM_MAX: u16 = 0x38;
    pub const QUEUE_NUM: u16 = 0x3C;
    pub const QUEUE_READY: u16 = 0x44;
    pub const QUEUE_DESC_LOW: u16 = 0x48;
    pub const QUEUE_DESC_HIGH: u16 = 0x4C;
    pub const QUEUE_DRIVER_LOW: u16 = 0x50;
    pub const QUEUE_DRIVER_HIGH: u16 = 0x54;
    pub const QUEUE_DEVICE_LOW: u16 = 0x58;
    pub const QUEUE_DEVICE_HIGH: u16 = 0x5C;
    pub const QUEUE_NOTIFY: u16 = 0x70;
    pub const INTERRUPT_STATUS: u16 = 0x60;
    pub const INTERRUPT_ACK: u16 = 0x64;
    pub const STATUS: u16 = 0x70;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioBlkError {
    DeviceNotReady,
    QueueTooSmall,
    OutOfFrames,
    InvalidFrame,
    ReadFailed,
    BadStatus,
}

/// One virtqueue descriptor (16 bytes, packed).
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

/// Available ring: driver writes used descriptor indices here.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct VirtqAvail {
    flags: u16,
    idx: u16,
    ring: [u16; QUEUE_SIZE as usize],
    used_event: u16,
}

/// Used ring: device writes completed descriptor indices here.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct VirtqUsed {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; QUEUE_SIZE as usize],
    avail_event: u16,
}

/// virtio-blk request header (12 bytes).
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct VirtioBlkReq {
    type_: u32,
    reserved: u32,
    sector: u64,
}

/// A configured virtio-blk device with one initialized virtqueue.
pub struct VirtioBlk {
    bar0: u64,
    desc_table: u64,
    avail_ring: u64,
    used_ring: u64,
    req_frame: u64,
    status_frame: u64,
    queue_idx: u16,
}

impl VirtioBlk {
    /// Configure the device: reset, negotiate features, set up one virtqueue.
    ///
    /// # Safety
    ///
    /// `pci` must be a valid virtio-blk device discovered via [`crate::pci::discover_virtio_blk`].
    /// The allocator must own free frames in the identity-mapped window.
    pub unsafe fn configure(
        pci: crate::pci::PciDevice,
        allocator: &mut PhysicalFrameAllocator,
    ) -> Result<Self, VirtioBlkError> {
        // Reset the device.
        unsafe { crate::console::outb((pci.bar0 as u16) + reg::STATUS, 0x00) };
        // Acknowledge + driver bits.
        unsafe { crate::console::outb((pci.bar0 as u16) + reg::STATUS, 0x03) };

        // Select queue 0.
        unsafe { crate::console::outb((pci.bar0 as u16) + reg::QUEUE_SEL, 0x00) };
        let queue_max = unsafe { crate::console::inb((pci.bar0 as u16) + reg::QUEUE_NUM_MAX) } as u16;
        if queue_max < QUEUE_SIZE {
            return Err(VirtioBlkError::QueueTooSmall);
        }
        unsafe { crate::console::outb((pci.bar0 as u16) + reg::QUEUE_NUM, QUEUE_SIZE as u8) };

        // Allocate physically contiguous memory for the ring structures plus
        // request header and status byte frames.
        let desc_frame = allocator
            .allocate_frame()
            .ok_or(VirtioBlkError::OutOfFrames)?;
        let avail_frame = allocator
            .allocate_frame()
            .ok_or(VirtioBlkError::OutOfFrames)?;
        let used_frame = allocator
            .allocate_frame()
            .ok_or(VirtioBlkError::OutOfFrames)?;
        let req_frame = allocator
            .allocate_frame()
            .ok_or(VirtioBlkError::OutOfFrames)?;
        let status_frame = allocator
            .allocate_frame()
            .ok_or(VirtioBlkError::OutOfFrames)?;
        for frame in [desc_frame, avail_frame, used_frame, req_frame, status_frame] {
            if frame % PAGE_SIZE != 0 {
                return Err(VirtioBlkError::InvalidFrame);
            }
            unsafe { core::ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE as usize) };
        }

        // SAFETY: All frames are page-aligned, zeroed, identity-accessible.
        unsafe {
            crate::console::outb(
                (pci.bar0 as u16) + reg::QUEUE_DESC_LOW,
                (desc_frame & 0xFF) as u8,
            );
            crate::console::outb(
                (pci.bar0 as u16) + reg::QUEUE_DESC_HIGH,
                ((desc_frame >> 8) & 0xFF) as u8,
            );
            crate::console::outb(
                (pci.bar0 as u16) + reg::QUEUE_DRIVER_LOW,
                (avail_frame & 0xFF) as u8,
            );
            crate::console::outb(
                (pci.bar0 as u16) + reg::QUEUE_DRIVER_HIGH,
                ((avail_frame >> 8) & 0xFF) as u8,
            );
            crate::console::outb(
                (pci.bar0 as u16) + reg::QUEUE_DEVICE_LOW,
                (used_frame & 0xFF) as u8,
            );
            crate::console::outb(
                (pci.bar0 as u16) + reg::QUEUE_DEVICE_HIGH,
                ((used_frame >> 8) & 0xFF) as u8,
            );
            // Mark the queue ready.
            crate::console::outb((pci.bar0 as u16) + reg::QUEUE_READY, 0x01);
        }

        // SAFETY: The device status is checked after queue setup.
        let status = unsafe { crate::console::inb((pci.bar0 as u16) + reg::STATUS) };
        if status & 0x04 == 0 {
            return Err(VirtioBlkError::DeviceNotReady);
        }

        Ok(VirtioBlk {
            bar0: pci.bar0,
            desc_table: desc_frame,
            avail_ring: avail_frame,
            used_ring: used_frame,
            req_frame,
            status_frame,
            queue_idx: 0,
        })
    }

    /// Read `count` sectors starting at `sector` into `buf` (identity-mapped).
    ///
    /// Returns the number of bytes read (count * 512) on success.
    ///
    /// # Safety
    ///
    /// `buf` must point to `count * 512` bytes of writable, identity-mapped
    /// memory. The device must have been configured via [`configure`].
    pub unsafe fn read_sectors(
        &mut self,
        sector: u64,
        count: u32,
        buf: *mut u8,
    ) -> Result<u64, VirtioBlkError> {
        if buf.is_null() {
            return Err(VirtioBlkError::ReadFailed);
        }

        // Build the request header in the pre-allocated frame.
        let req = VirtioBlkReq {
            type_: VIRTIO_BLK_T_IN,
            reserved: 0,
            sector,
        };
        // SAFETY: `self.req_frame` is a fresh, zeroed, identity-accessible frame.
        unsafe {
            core::ptr::write_unaligned(self.req_frame as *mut VirtioBlkReq, req);
        }

        // Data buffer: the caller-provided buffer must be identity-mapped.
        let data_phys = buf as u64;

        // Status byte: pre-allocated frame, preset to 0xFF (not yet done).
        unsafe { core::ptr::write_volatile(self.status_frame as *mut u8, 0xFF) };

        // Descriptor chain: req (read) -> data (write) -> status (write).
        let desc: [VirtqDesc; 3] = [
            VirtqDesc {
                addr: self.req_frame,
                len: core::mem::size_of::<VirtioBlkReq>() as u32,
                flags: 0,
                next: 1,
            },
            VirtqDesc {
                addr: data_phys,
                len: ((count as u64) * VIRTIO_BLK_SECTOR_SIZE) as u32,
                flags: VIRTQ_DESC_F_WRITE,
                next: 2,
            },
            VirtqDesc {
                addr: self.status_frame,
                len: 1,
                flags: VIRTQ_DESC_F_WRITE,
                next: 0xFFFF,
            },
        ];

        // SAFETY: The descriptor table frame is zeroed and owned by this driver.
        unsafe {
            core::ptr::write_bytes(self.desc_table as *mut u8, 0, PAGE_SIZE as usize);
            for (i, d) in desc.iter().enumerate() {
                core::ptr::write_unaligned(
                    (self.desc_table + (i as u64) * core::mem::size_of::<VirtqDesc>() as u64) as *mut VirtqDesc,
                    *d,
                );
            }
            // Available ring: offer descriptor 0.
            let avail = self.avail_ring as *mut VirtqAvail;
            core::ptr::write_unaligned(avail, VirtqAvail {
                flags: 0,
                idx: 0,
                ring: [0; QUEUE_SIZE as usize],
                used_event: 0,
            });
            (*avail).ring[0] = 0;
            (*avail).idx = 1;
            // Used ring: clear.
            let used = self.used_ring as *mut VirtqUsed;
            core::ptr::write_unaligned(used, VirtqUsed {
                flags: 0,
                idx: 0,
                ring: [VirtqUsedElem { id: 0, len: 0 }; QUEUE_SIZE as usize],
                avail_event: 0,
            });
            // Notify the device.
            crate::console::outb((self.bar0 as u16) + reg::QUEUE_NOTIFY, 0x00);
        }

        // Poll for completion (no interrupt support yet).
        let mut attempts = 0;
        while attempts < 1_000_000 {
            // SAFETY: The used ring frame is identity-accessible.
            let used_idx = unsafe {
                let used = &*(self.used_ring as *const VirtqUsed);
                used.idx
            };
            if used_idx != 0 {
                // SAFETY: The status byte is identity-accessible.
                let status = unsafe { core::ptr::read_volatile(self.status_frame as *const u8) };
                if status == VIRTIO_BLK_S_OK {
                    return Ok((count as u64) * VIRTIO_BLK_SECTOR_SIZE);
                }
                return Err(VirtioBlkError::BadStatus);
            }
            attempts += 1;
        }

        Err(VirtioBlkError::ReadFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtio_blk_constants() {
        assert_eq!(VIRTIO_BLK_SECTOR_SIZE, 512);
        assert_eq!(VIRTQ_DESC_F_NEXT, 1);
        assert_eq!(VIRTQ_DESC_F_WRITE, 2);
        assert_eq!(VIRTIO_BLK_T_IN, 0);
        assert_eq!(VIRTIO_BLK_S_OK, 0);
        assert_eq!(QUEUE_SIZE, 8);
    }

    #[test]
    fn req_header_size_and_layout() {
        // The packed header occupies 16 bytes due to u64 alignment padding,
        // but only the first 12 bytes (type_, reserved, sector) are significant.
        assert!(core::mem::size_of::<VirtioBlkReq>() >= 12);
        assert_eq!(core::mem::offset_of!(VirtioBlkReq, type_), 0);
        assert_eq!(core::mem::offset_of!(VirtioBlkReq, sector), 8);
    }

    #[test]
    fn desc_size_is_16_bytes() {
        assert_eq!(core::mem::size_of::<VirtqDesc>(), 16);
    }
}
