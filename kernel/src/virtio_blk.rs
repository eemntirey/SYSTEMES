//! Synchronous transitional virtio-blk driver using a legacy PCI I/O BAR.

use crate::memory::{PhysicalFrameAllocator, PAGE_SIZE};
use core::sync::atomic::{compiler_fence, Ordering};

const SECTOR_SIZE: usize = 512;
const DESCRIPTOR_COUNT: usize = 3;
const DMA_POLL_LIMIT: usize = 100_000_000;

const DESC_F_NEXT: u16 = 1;
const DESC_F_WRITE: u16 = 2;
const REQUEST_IN: u32 = 0;
const REQUEST_OUT: u32 = 1;
const STATUS_OK: u8 = 0;
const STATUS_IOERR: u8 = 1;
const STATUS_UNSUPP: u8 = 2;

mod reg {
    pub const HOST_FEATURES: u16 = 0x00;
    pub const GUEST_FEATURES: u16 = 0x04;
    pub const QUEUE_PFN: u16 = 0x08;
    pub const QUEUE_NUM_MAX: u16 = 0x0C;
    pub const QUEUE_SEL: u16 = 0x0E;
    pub const QUEUE_NOTIFY: u16 = 0x10;
    pub const STATUS: u16 = 0x12;
    pub const CAPACITY: u16 = 0x14;
    pub const GUEST_PAGE_SIZE: u16 = 0x28;
    pub const QUEUE_ALIGN: u16 = 0x20;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioBlkError {
    UnsupportedDevice,
    InvalidBar,
    DeviceNotReady,
    QueueTooSmall,
    InvalidQueueSize,
    OutOfFrames,
    InvalidFrame,
    SectorOutOfRange,
    RequestFailed,
    IoError,
    UnsupportedRequest,
    DmaTimeout,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct VirtqDesc {
    address: u64,
    length: u32,
    flags: u16,
    next: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct BlkRequest {
    request_type: u32,
    reserved: u32,
    sector: u64,
}

pub struct VirtioBlk {
    io_base: u16,
    queue_memory: u64,
    request_frame: u64,
    status_frame: u64,
    queue_size: u16,
    available_index: u16,
    used_index: u16,
    capacity_sectors: u64,
}

impl VirtioBlk {
    /// Reset and initialize one legacy virtqueue for a discovered PCI device.
    ///
    /// # Safety
    ///
    /// PCI I/O ports and all allocator-returned frames must be accessible and
    /// identity-mapped. `pci` must be returned by `discover_virtio_blk`.
    pub unsafe fn configure(
        pci: crate::pci::PciDevice,
        allocator: &mut PhysicalFrameAllocator,
    ) -> Result<Self, VirtioBlkError> {
        if pci.vendor_id != 0x1AF4 || pci.device_id != 0x1001 {
            return Err(VirtioBlkError::UnsupportedDevice);
        }
        let io_base = u16::try_from(pci.bar0).map_err(|_| VirtioBlkError::InvalidBar)?;
        if io_base == 0 || io_base.checked_add(0x34).is_none() {
            return Err(VirtioBlkError::InvalidBar);
        }

        unsafe { crate::pci::enable_io_and_bus_master(pci) };
        unsafe { crate::console::outb(io_base + reg::STATUS, 0) };
        if unsafe { crate::console::inb(io_base + reg::STATUS) } != 0 {
            return Err(VirtioBlkError::DeviceNotReady);
        }
        unsafe { crate::console::outb(io_base + reg::STATUS, 1) };
        unsafe { crate::console::outb(io_base + reg::STATUS, 3) };

        let offered_features = unsafe { crate::console::inl(io_base + reg::HOST_FEATURES) };
        let supported_features = 0u32;
        if supported_features & !offered_features != 0 {
            return Err(VirtioBlkError::DeviceNotReady);
        }
        unsafe { crate::console::outl(io_base + reg::GUEST_FEATURES, supported_features) };
        unsafe { crate::console::outb(io_base + reg::STATUS, 0x0B) };
        if unsafe { crate::console::inb(io_base + reg::STATUS) } & 0x08 == 0 {
            return Err(VirtioBlkError::DeviceNotReady);
        }

        unsafe { crate::console::outw(io_base + reg::QUEUE_SEL, 0) };
        let queue_size = unsafe { crate::console::inw(io_base + reg::QUEUE_NUM_MAX) };
        if queue_size < DESCRIPTOR_COUNT as u16 {
            return Err(VirtioBlkError::QueueTooSmall);
        }
        if !queue_size.is_power_of_two() {
            return Err(VirtioBlkError::InvalidQueueSize);
        }
        unsafe { crate::console::outl(io_base + reg::GUEST_PAGE_SIZE, PAGE_SIZE as u32) };
        unsafe { crate::console::outl(io_base + reg::QUEUE_ALIGN, PAGE_SIZE as u32) };

        let queue_pages = queue_memory_pages(queue_size);
        let queue_memory = allocator
            .allocate_contiguous_frames(queue_pages)
            .ok_or(VirtioBlkError::OutOfFrames)?;
        let request_frame = allocator
            .allocate_frame()
            .ok_or(VirtioBlkError::OutOfFrames)?;
        let status_frame = allocator
            .allocate_frame()
            .ok_or(VirtioBlkError::OutOfFrames)?;
        for frame in [queue_memory, request_frame, status_frame] {
            if frame % PAGE_SIZE != 0
                || frame
                    .checked_add(PAGE_SIZE)
                    .is_none_or(|end| end > crate::paging::IDENTITY_MAP_BYTES)
            {
                return Err(VirtioBlkError::InvalidFrame);
            }
        }
        if queue_memory
            .checked_add(queue_pages as u64 * PAGE_SIZE)
            .is_none_or(|end| end > crate::paging::IDENTITY_MAP_BYTES)
        {
            return Err(VirtioBlkError::InvalidFrame);
        }
        unsafe {
            core::ptr::write_bytes(
                queue_memory as *mut u8,
                0,
                (queue_pages as u64 * PAGE_SIZE) as usize,
            );
            core::ptr::write_bytes(request_frame as *mut u8, 0, PAGE_SIZE as usize);
            core::ptr::write_bytes(status_frame as *mut u8, 0, PAGE_SIZE as usize);
        }

        let queue_pfn = queue_memory / PAGE_SIZE;
        let queue_pfn = u32::try_from(queue_pfn).map_err(|_| VirtioBlkError::InvalidFrame)?;
        unsafe { crate::console::outl(io_base + reg::QUEUE_PFN, queue_pfn) };
        let low_capacity = unsafe { crate::console::inl(io_base + reg::CAPACITY) };
        let high_capacity = unsafe { crate::console::inl(io_base + reg::CAPACITY + 4) };
        let capacity_sectors = ((high_capacity as u64) << 32) | low_capacity as u64;
        if capacity_sectors == 0 {
            return Err(VirtioBlkError::DeviceNotReady);
        }

        unsafe { crate::console::outb(io_base + reg::STATUS, 0x0F) };
        if unsafe { crate::console::inb(io_base + reg::STATUS) } & 0x04 == 0 {
            return Err(VirtioBlkError::DeviceNotReady);
        }

        Ok(Self {
            io_base,
            queue_memory,
            request_frame,
            status_frame,
            queue_size,
            available_index: 0,
            used_index: 0,
            capacity_sectors,
        })
    }

    pub fn capacity_sectors(&self) -> u64 { self.capacity_sectors }

    /// Read one 512-byte sector through the device DMA virtqueue.
    ///
    /// # Safety
    ///
    /// The caller must pass the allocator used to configure this device. The
    /// destination must be a writable kernel buffer that does not alias DMA memory.
    pub unsafe fn read_sector(
        &mut self,
        sector: u64,
        destination: &mut [u8; SECTOR_SIZE],
        allocator: &mut PhysicalFrameAllocator,
    ) -> Result<(), VirtioBlkError> {
        if sector >= self.capacity_sectors {
            return Err(VirtioBlkError::SectorOutOfRange);
        }
        let data_frame = allocator
            .allocate_frame()
            .ok_or(VirtioBlkError::OutOfFrames)?;
        if data_frame % PAGE_SIZE != 0
            || data_frame
                .checked_add(PAGE_SIZE)
                .is_none_or(|end| end > crate::paging::IDENTITY_MAP_BYTES)
        {
            allocator
                .free_frame(data_frame)
                .map_err(|_| VirtioBlkError::InvalidFrame)?;
            return Err(VirtioBlkError::InvalidFrame);
        }
        unsafe { core::ptr::write_bytes(data_frame as *mut u8, 0, PAGE_SIZE as usize) };
        let result = unsafe { self.submit_read(sector, data_frame) };
        if result.is_ok() {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    data_frame as *const u8,
                    destination.as_mut_ptr(),
                    SECTOR_SIZE,
                );
            }
        }
        allocator
            .free_frame(data_frame)
            .map_err(|_| VirtioBlkError::InvalidFrame)?;
        result
    }


    /// Write one 512-byte sector through the device DMA virtqueue.
    ///
    /// # Safety
    /// The allocator must be the one used to configure this device.
    pub unsafe fn write_sector(
        &mut self,
        sector: u64,
        source: &[u8; SECTOR_SIZE],
        allocator: &mut PhysicalFrameAllocator,
    ) -> Result<(), VirtioBlkError> {
        if sector >= self.capacity_sectors {
            return Err(VirtioBlkError::SectorOutOfRange);
        }
        let data_frame = allocator.allocate_frame().ok_or(VirtioBlkError::OutOfFrames)?;
        if data_frame % PAGE_SIZE != 0
            || data_frame.checked_add(PAGE_SIZE).is_none_or(|end| end > crate::paging::IDENTITY_MAP_BYTES)
        {
            allocator.free_frame(data_frame).map_err(|_| VirtioBlkError::InvalidFrame)?;
            return Err(VirtioBlkError::InvalidFrame);
        }
        unsafe {
            core::ptr::write_bytes(data_frame as *mut u8, 0, PAGE_SIZE as usize);
            core::ptr::copy_nonoverlapping(source.as_ptr(), data_frame as *mut u8, SECTOR_SIZE);
        }
        let result = unsafe { self.submit_write(sector, data_frame) };
        allocator.free_frame(data_frame).map_err(|_| VirtioBlkError::InvalidFrame)?;
        result
    }

    unsafe fn submit_read(&mut self, sector: u64, data_frame: u64) -> Result<(), VirtioBlkError> {
        let request = BlkRequest {
            request_type: REQUEST_IN,
            reserved: 0,
            sector,
        };
        unsafe {
            core::ptr::write_unaligned(self.request_frame as *mut BlkRequest, request);
            core::ptr::write_volatile(self.status_frame as *mut u8, 0xFF);
        }

        let descriptors: [VirtqDesc; DESCRIPTOR_COUNT] = [
            VirtqDesc {
                address: self.request_frame,
                length: core::mem::size_of::<BlkRequest>() as u32,
                flags: DESC_F_NEXT,
                next: 1,
            },
            VirtqDesc {
                address: data_frame,
                length: SECTOR_SIZE as u32,
                flags: DESC_F_NEXT | DESC_F_WRITE,
                next: 2,
            },
            VirtqDesc {
                address: self.status_frame,
                length: 1,
                flags: DESC_F_WRITE,
                next: 0,
            },
        ];
        let descriptor_table = self.queue_memory as *mut u8;
        for (index, descriptor) in descriptors.iter().enumerate() {
            unsafe {
                core::ptr::write_unaligned(
                    descriptor_table
                        .add(index * core::mem::size_of::<VirtqDesc>())
                        as *mut VirtqDesc,
                    *descriptor,
                );
            }
        }

        let avail_offset = core::mem::size_of::<VirtqDesc>() * self.queue_size as usize;
        let avail_index = self.available_index;
        let avail_slot = (avail_index % self.queue_size) as usize;
        unsafe {
            core::ptr::write_volatile((self.queue_memory + avail_offset as u64 + 4 + (avail_slot * 2) as u64) as *mut u16, 0);
            core::ptr::write_volatile((self.queue_memory + avail_offset as u64 + 2) as *mut u16, avail_index.wrapping_add(1));
        }
        self.available_index = avail_index.wrapping_add(1);
        compiler_fence(Ordering::Release);
        unsafe { crate::console::outw(self.io_base + reg::QUEUE_NOTIFY, 0) };

        let used_offset = used_ring_offset(self.queue_size);
        for _ in 0..DMA_POLL_LIMIT {
            let current_used = unsafe {
                core::ptr::read_volatile((self.queue_memory + used_offset as u64 + 2) as *const u16)
            };
            if current_used != self.used_index {
                if current_used != self.used_index.wrapping_add(1) {
                    return Err(VirtioBlkError::RequestFailed);
                }
                compiler_fence(Ordering::Acquire);
                let used_element_offset = used_offset + 4 + (self.used_index % self.queue_size) as usize * 8;
                let used_id = unsafe {
                    core::ptr::read_volatile(
                        (self.queue_memory + used_element_offset as u64) as *const u32,
                    )
                };
                let used_length = unsafe {
                    core::ptr::read_volatile(
                        (self.queue_memory + used_element_offset as u64 + 4) as *const u32,
                    )
                };
                self.used_index = current_used;
                if used_id != 0 || used_length < (SECTOR_SIZE + 1) as u32 {
                    return Err(VirtioBlkError::RequestFailed);
                }
                let status = unsafe { core::ptr::read_volatile(self.status_frame as *const u8) };
                return match status {
                    STATUS_OK => Ok(()),
                    STATUS_IOERR => Err(VirtioBlkError::IoError),
                    STATUS_UNSUPP => Err(VirtioBlkError::UnsupportedRequest),
                    _ => Err(VirtioBlkError::RequestFailed),
                };
            }
            core::hint::spin_loop();
        }
        Err(VirtioBlkError::DmaTimeout)
    }

    unsafe fn submit_write(&mut self, sector: u64, data_frame: u64) -> Result<(), VirtioBlkError> {
        let request = BlkRequest {
            request_type: REQUEST_OUT,
            reserved: 0,
            sector,
        };
        unsafe {
            core::ptr::write_unaligned(self.request_frame as *mut BlkRequest, request);
            core::ptr::write_volatile(self.status_frame as *mut u8, 0xFF);
        }

        let descriptors: [VirtqDesc; DESCRIPTOR_COUNT] = [
            VirtqDesc {
                address: self.request_frame,
                length: core::mem::size_of::<BlkRequest>() as u32,
                flags: DESC_F_NEXT,
                next: 1,
            },
            VirtqDesc {
                address: data_frame,
                length: SECTOR_SIZE as u32,
                flags: DESC_F_NEXT,
                next: 2,
            },
            VirtqDesc {
                address: self.status_frame,
                length: 1,
                flags: DESC_F_WRITE,
                next: 0,
            },
        ];
        let descriptor_table = self.queue_memory as *mut u8;
        for (index, descriptor) in descriptors.iter().enumerate() {
            unsafe {
                core::ptr::write_unaligned(
                    descriptor_table
                        .add(index * core::mem::size_of::<VirtqDesc>())
                        as *mut VirtqDesc,
                    *descriptor,
                );
            }
        }

        let avail_offset = core::mem::size_of::<VirtqDesc>() * self.queue_size as usize;
        let avail_index = self.available_index;
        let avail_slot = (avail_index % self.queue_size) as usize;
        unsafe {
            core::ptr::write_volatile((self.queue_memory + avail_offset as u64 + 4 + (avail_slot * 2) as u64) as *mut u16, 0);
            core::ptr::write_volatile((self.queue_memory + avail_offset as u64 + 2) as *mut u16, avail_index.wrapping_add(1));
        }
        self.available_index = avail_index.wrapping_add(1);
        compiler_fence(Ordering::Release);
        unsafe { crate::console::outw(self.io_base + reg::QUEUE_NOTIFY, 0) };

        let used_offset = used_ring_offset(self.queue_size);
        for _ in 0..DMA_POLL_LIMIT {
            let current_used = unsafe {
                core::ptr::read_volatile((self.queue_memory + used_offset as u64 + 2) as *const u16)
            };
            if current_used != self.used_index {
                if current_used != self.used_index.wrapping_add(1) {
                    return Err(VirtioBlkError::RequestFailed);
                }
                compiler_fence(Ordering::Acquire);
                let used_element_offset = used_offset + 4 + (self.used_index % self.queue_size) as usize * 8;
                let used_id = unsafe {
                    core::ptr::read_volatile(
                        (self.queue_memory + used_element_offset as u64) as *const u32,
                    )
                };
                let used_length = unsafe {
                    core::ptr::read_volatile(
                        (self.queue_memory + used_element_offset as u64 + 4) as *const u32,
                    )
                };
                self.used_index = current_used;
                if used_id != 0 || used_length < 1 {
                    return Err(VirtioBlkError::RequestFailed);
                }
                let status = unsafe { core::ptr::read_volatile(self.status_frame as *const u8) };
                return match status {
                    STATUS_OK => Ok(()),
                    STATUS_IOERR => Err(VirtioBlkError::IoError),
                    STATUS_UNSUPP => Err(VirtioBlkError::UnsupportedRequest),
                    _ => Err(VirtioBlkError::RequestFailed),
                };
            }
            core::hint::spin_loop();
        }
        Err(VirtioBlkError::DmaTimeout)
    }
}


/// Exclusive adapter exposing the legacy virtio device through the block layer.
/// It deliberately reports no durability barrier until VIRTIO_BLK_F_FLUSH is
/// negotiated and implemented by this driver.
pub struct VirtioBlockDevice<'a> {
    pub device: &'a mut VirtioBlk,
    pub allocator: &'a mut PhysicalFrameAllocator,
}

impl crate::block::BlockDevice for VirtioBlockDevice<'_> {
    fn capacity_blocks(&self) -> u64 { self.device.capacity_sectors() }

    fn read_block(
        &mut self,
        lba: u64,
        out: &mut [u8; crate::block::BLOCK_SIZE],
    ) -> Result<(), crate::block::BlockError> {
        let mut sector = [0u8; SECTOR_SIZE];
        // SAFETY: this adapter exclusively borrows the configured device and its allocator.
        unsafe { self.device.read_sector(lba, &mut sector, self.allocator) }
            .map_err(|_| crate::block::BlockError::Io)?;
        out.copy_from_slice(&sector);
        Ok(())
    }

    fn write_block(
        &mut self,
        lba: u64,
        data: &[u8; crate::block::BLOCK_SIZE],
    ) -> Result<(), crate::block::BlockError> {
        let mut sector = [0u8; SECTOR_SIZE];
        sector.copy_from_slice(data);
        // SAFETY: this adapter exclusively borrows the configured device and its allocator.
        unsafe { self.device.write_sector(lba, &sector, self.allocator) }
            .map_err(|_| crate::block::BlockError::Io)
    }

    fn flush(&mut self) -> Result<(), crate::block::BlockError> {
        Err(crate::block::BlockError::Unsupported)
    }
}

fn used_ring_offset(queue_size: u16) -> usize {
    let descriptor_bytes = core::mem::size_of::<VirtqDesc>() * queue_size as usize;
    let available_bytes = 4 + 2 * queue_size as usize + 2;
    let end = descriptor_bytes + available_bytes;
    (end + PAGE_SIZE as usize - 1) & !(PAGE_SIZE as usize - 1)
}

fn queue_memory_pages(queue_size: u16) -> usize {
    let used_offset = used_ring_offset(queue_size);
    let used_end = used_offset + 4 + 8 * queue_size as usize + 2;
    (used_end + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_queue_regions_fit_in_the_allocated_contiguous_pages() {
        assert_eq!(queue_memory_pages(8), 2);
        let queue_size = 256;
        let used = used_ring_offset(queue_size);
        let used_end = used + 4 + 8 * queue_size as usize + 2;
        assert_eq!(used, 2 * PAGE_SIZE as usize);
        assert_eq!(queue_memory_pages(queue_size), 3);
        assert!(used_end <= queue_memory_pages(queue_size) * PAGE_SIZE as usize);
    }

    #[test]
    fn descriptor_flags_form_read_request_chain() {
        let flags = [DESC_F_NEXT, DESC_F_NEXT | DESC_F_WRITE, DESC_F_WRITE];
        assert_eq!(flags, [1, 3, 2]);
        assert_eq!(core::mem::size_of::<VirtqDesc>(), 16);
        assert_eq!(core::mem::size_of::<BlkRequest>(), 16);
        assert_eq!(DESCRIPTOR_COUNT, 3);
    }
}
