//! Single-queue virtio-blk driver for transitional PCI devices.

use core::{
    cell::UnsafeCell,
    ptr::{read_volatile, write_volatile},
    sync::atomic::{Ordering, compiler_fence},
};

use crate::console::{inb, outb};

const PCI_CONFIG_ADDRESS: u16 = 0x0CF8;
const PCI_CONFIG_DATA: u16 = 0x0CFC;
const PCI_COMMAND: u8 = 0x04;
const PCI_BAR0: u8 = 0x10;
const PCI_VENDOR_VIRTIO: u16 = 0x1AF4;
const PCI_DEVICE_BLOCK_LEGACY: u16 = 0x1001;

const IO_DEVICE_FEATURES: u16 = 0x00;
const IO_GUEST_FEATURES: u16 = 0x04;
const IO_QUEUE_ADDRESS: u16 = 0x08;
const IO_QUEUE_SIZE: u16 = 0x0C;
const IO_QUEUE_SELECT: u16 = 0x0E;
const IO_QUEUE_NOTIFY: u16 = 0x10;
const IO_DEVICE_STATUS: u16 = 0x12;
const IO_DEVICE_CONFIG: u16 = 0x14;

const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_DEVICE_NEEDS_RESET: u8 = 64;
const STATUS_FAILED: u8 = 128;

const QUEUE_SIZE: usize = 128;
const QUEUE_BYTES: usize = 3 * 4096;
const DESC_NEXT: u16 = 1;
const DESC_WRITE: u16 = 2;
const REQUEST_IN: u32 = 0;
const REQUEST_OUT: u32 = 1;
pub const SECTOR_SIZE: usize = 512;
const POLL_LIMIT: usize = 100_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioBlkError {
    DeviceNotFound,
    UnsupportedBar,
    InvalidQueueSize,
    QueueAddressOutOfRange,
    DeviceNeedsReset,
    SectorOutOfRange,
    RequestTimedOut,
    InvalidCompletion,
    IoError,
    UnsupportedRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PciDevice {
    io_base: u16,
}

#[repr(C, align(4096))]
struct QueueMemory(UnsafeCell<[u8; QUEUE_BYTES]>);

unsafe impl Sync for QueueMemory {}

#[repr(C, align(16))]
struct RequestMemory(UnsafeCell<[u8; 16 + SECTOR_SIZE + 1]>);

unsafe impl Sync for RequestMemory {}

static QUEUE: QueueMemory = QueueMemory(UnsafeCell::new([0; QUEUE_BYTES]));
static REQUEST: RequestMemory = RequestMemory(UnsafeCell::new([0; 16 + SECTOR_SIZE + 1]));

fn pci_config_address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | (u32::from(bus) << 16)
        | (u32::from(device) << 11)
        | (u32::from(function) << 8)
        | u32::from(offset & 0xFC)
}

fn pci_read_u32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    // SAFETY: PCI configuration mechanism #1 is used during single-core boot.
    unsafe {
        outl(PCI_CONFIG_ADDRESS, pci_config_address(bus, device, function, offset));
        inl(PCI_CONFIG_DATA)
    }
}

fn pci_write_command(bus: u8, device: u8, function: u8, command: u16) {
    // SAFETY: PCI configuration mechanism #1 is used during single-core boot.
    unsafe {
        outl(PCI_CONFIG_ADDRESS, pci_config_address(bus, device, function, PCI_COMMAND));
        outw(PCI_CONFIG_DATA, command);
    }
}

fn find_device() -> Result<PciDevice, VirtioBlkError> {
    for bus in 0..=u8::MAX {
        for device in 0..32u8 {
            let first = pci_read_u32(bus, device, 0, 0);
            if first as u16 == u16::MAX {
                continue;
            }
            let header = pci_read_u32(bus, device, 0, 0x0C);
            let functions: u8 = if (header >> 23) & 1 == 0 { 1 } else { 8 };
            for function in 0..functions {
                let identity = pci_read_u32(bus, device, function, 0);
                let vendor = identity as u16;
                let id = (identity >> 16) as u16;
                if vendor == PCI_VENDOR_VIRTIO && id == PCI_DEVICE_BLOCK_LEGACY {
                    let bar = pci_read_u32(bus, device, function, PCI_BAR0);
                    if bar & 1 == 0 || bar & !0x3 != 0 {
                        return Err(VirtioBlkError::UnsupportedBar);
                    }
                    let io_base = (bar & !0x3) as u16;
                    if io_base == 0 || io_base > u16::MAX - IO_DEVICE_CONFIG - 7 {
                        return Err(VirtioBlkError::UnsupportedBar);
                    }

                    let command = pci_read_u32(bus, device, function, PCI_COMMAND) as u16;
                    pci_write_command(bus, device, function, command | 0x5);
                    return Ok(PciDevice { io_base });
                }
            }
        }
    }
    Err(VirtioBlkError::DeviceNotFound)
}

pub struct VirtioBlkDriver {
    io_base: u16,
    queue_size: u16,
    capacity_sectors: u64,
    available_index: u16,
}

impl VirtioBlkDriver {
    /// Locate and initialize a transitional PCI virtio-blk device.
    ///
    /// This driver requires a legacy I/O BAR and a single CPU. DMA uses the
    /// kernel's current identity mapping and is limited to addresses below 4 GiB.
    pub fn initialize() -> Result<Self, VirtioBlkError> {
        let device = find_device()?;
        let mut driver = Self {
            io_base: device.io_base,
            queue_size: 0,
            capacity_sectors: 0,
            available_index: 0,
        };
        if let Err(error) = driver.initialize_device() {
            let status = driver.read_u8(IO_DEVICE_STATUS);
            driver.write_u8(IO_DEVICE_STATUS, status | STATUS_FAILED);
            return Err(error);
        }
        Ok(driver)
    }

    fn initialize_device(&mut self) -> Result<(), VirtioBlkError> {
        self.write_u8(IO_DEVICE_STATUS, 0);
        self.write_u8(IO_DEVICE_STATUS, STATUS_ACKNOWLEDGE);
        self.write_u8(IO_DEVICE_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);

        let _host_features = self.read_u32(IO_DEVICE_FEATURES);
        self.write_u32(IO_GUEST_FEATURES, 0);

        self.write_u16(IO_QUEUE_SELECT, 0);
        let queue_size = self.read_u16(IO_QUEUE_SIZE);
        if queue_size == 0 || !queue_size.is_power_of_two() || usize::from(queue_size) > QUEUE_SIZE {
            return Err(VirtioBlkError::InvalidQueueSize);
        }
        self.queue_size = queue_size;

        let queue_address = QUEUE.0.get().cast::<u8>();
        let queue_physical = queue_address as u64;
        if queue_physical & 0xFFF != 0 || queue_physical >> 32 != 0 {
            return Err(VirtioBlkError::QueueAddressOutOfRange);
        }
        // SAFETY: The statically allocated queue is aligned, identity-mapped, and below 4 GiB.
        unsafe { core::ptr::write_bytes(queue_address, 0, QUEUE_BYTES) };
        compiler_fence(Ordering::SeqCst);
        self.write_u32(IO_QUEUE_ADDRESS, (queue_physical >> 12) as u32);

        let capacity_low = self.read_u32(IO_DEVICE_CONFIG);
        let capacity_high = self.read_u32(IO_DEVICE_CONFIG + 4);
        self.capacity_sectors = u64::from(capacity_low) | (u64::from(capacity_high) << 32);
        if self.capacity_sectors == 0 {
            return Err(VirtioBlkError::UnsupportedBar);
        }

        self.write_u8(
            IO_DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK,
        );
        Ok(())
    }

    pub const fn capacity_sectors(&self) -> u64 {
        self.capacity_sectors
    }

    pub fn read_sector(
        &mut self,
        sector: u64,
        buffer: &mut [u8; SECTOR_SIZE],
    ) -> Result<(), VirtioBlkError> {
        self.submit(REQUEST_IN, sector, Some(buffer))
    }

    pub fn write_sector(
        &mut self,
        sector: u64,
        buffer: &[u8; SECTOR_SIZE],
    ) -> Result<(), VirtioBlkError> {
        let mut copy = [0; SECTOR_SIZE];
        copy.copy_from_slice(buffer);
        self.submit(REQUEST_OUT, sector, Some(&mut copy))
    }

    fn submit(
        &mut self,
        request_type: u32,
        sector: u64,
        data: Option<&mut [u8; SECTOR_SIZE]>,
    ) -> Result<(), VirtioBlkError> {
        if sector >= self.capacity_sectors {
            return Err(VirtioBlkError::SectorOutOfRange);
        }
        if self.read_u8(IO_DEVICE_STATUS) & STATUS_DEVICE_NEEDS_RESET != 0 {
            return Err(VirtioBlkError::DeviceNeedsReset);
        }

        let queue = QUEUE.0.get().cast::<u8>();
        let desc = queue.cast::<u8>();
        let avail_offset = usize::from(self.queue_size) * 16;
        let used_offset = align_up(avail_offset + 4 + usize::from(self.queue_size) * 2, 4096);
        let available_ring = unsafe { queue.add(avail_offset) };
        let used_ring = unsafe { queue.add(used_offset) };
        let request = REQUEST.0.get().cast::<u8>();
        let data_address = unsafe { request.add(16) };
        let status_address = unsafe { request.add(16 + SECTOR_SIZE) };

        // SAFETY: Request memory and queue are private static DMA buffers; device is initialized.
        unsafe {
            write_volatile(request.cast::<u32>(), request_type);
            write_volatile(request.add(4).cast::<u32>(), 0);
            write_volatile(request.add(8).cast::<u64>(), sector);
            write_volatile(status_address, 0xFF);

            if request_type == REQUEST_OUT {
                let source = data.as_deref().ok_or(VirtioBlkError::UnsupportedRequest)?;
                core::ptr::copy_nonoverlapping(source.as_ptr(), data_address, SECTOR_SIZE);
            }

            write_descriptor(desc, request as u64, 16, DESC_NEXT, 1);
            write_descriptor(
                desc.add(16),
                data_address as u64,
                SECTOR_SIZE as u32,
                DESC_NEXT | if request_type == REQUEST_IN { DESC_WRITE } else { 0 },
                2,
            );
            write_descriptor(desc.add(32), status_address as u64, 1, DESC_WRITE, 0);

            let ring_index = self.available_index % self.queue_size;
            write_volatile(
                available_ring.add(4).cast::<u16>().add(usize::from(ring_index)),
                0,
            );
            compiler_fence(Ordering::Release);
            self.available_index = self.available_index.wrapping_add(1);
            write_volatile(available_ring.add(2).cast::<u16>(), self.available_index);
            compiler_fence(Ordering::SeqCst);
            let initial_used = read_volatile(used_ring.add(2).cast::<u16>());
            compiler_fence(Ordering::SeqCst);
            self.write_u16(IO_QUEUE_NOTIFY, 0);
            let mut completed = false;
            for _ in 0..POLL_LIMIT {
                compiler_fence(Ordering::Acquire);
                if read_volatile(used_ring.add(2).cast::<u16>()) != initial_used {
                    completed = true;
                    break;
                }
                core::hint::spin_loop();
            }
            if !completed {
                return Err(VirtioBlkError::RequestTimedOut);
            }
            let used_element = used_ring.add(4 + usize::from(initial_used % self.queue_size) * 8);
            let completed_id = read_volatile(used_element.cast::<u32>());
            let completed_length = read_volatile(used_element.add(4).cast::<u32>());
            if completed_id != 0
                || completed_length != if request_type == REQUEST_IN { 513 } else { 1 }
            {
                return Err(VirtioBlkError::InvalidCompletion);
            }
            match read_volatile(status_address) {
                0 => {
                    if request_type == REQUEST_IN {
                        let destination = data.ok_or(VirtioBlkError::UnsupportedRequest)?;
                        core::ptr::copy_nonoverlapping(
                            data_address,
                            destination.as_mut_ptr(),
                            SECTOR_SIZE,
                        );
                    }
                    Ok(())
                }
                1 => Err(VirtioBlkError::IoError),
                _ => Err(VirtioBlkError::UnsupportedRequest),
            }
        }
    }

    fn read_u8(&self, offset: u16) -> u8 {
        // SAFETY: The BAR was validated as an I/O range during PCI discovery.
        unsafe { inb(self.io_base + offset) }
    }

    fn read_u16(&self, offset: u16) -> u16 {
        // SAFETY: The BAR was validated as an I/O range during PCI discovery.
        unsafe { inw(self.io_base + offset) }
    }

    fn read_u32(&self, offset: u16) -> u32 {
        // SAFETY: The BAR was validated as an I/O range during PCI discovery.
        unsafe { inl(self.io_base + offset) }
    }

    fn write_u8(&self, offset: u16, value: u8) {
        // SAFETY: The BAR was validated as an I/O range during PCI discovery.
        unsafe { outb(self.io_base + offset, value) }
    }

    fn write_u16(&self, offset: u16, value: u16) {
        // SAFETY: The BAR was validated as an I/O range during PCI discovery.
        unsafe { outw(self.io_base + offset, value) }
    }

    fn write_u32(&self, offset: u16, value: u32) {
        // SAFETY: The BAR was validated as an I/O range during PCI discovery.
        unsafe { outl(self.io_base + offset, value) }
    }
}

fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

unsafe fn write_descriptor(
    descriptor: *mut u8,
    address: u64,
    length: u32,
    flags: u16,
    next: u16,
) {
    // SAFETY: The caller provides one 16-byte aligned virtqueue descriptor.
    unsafe {
        write_volatile(descriptor.cast::<u64>(), address);
        write_volatile(descriptor.add(8).cast::<u32>(), length);
        write_volatile(descriptor.add(12).cast::<u16>(), flags);
        write_volatile(descriptor.add(14).cast::<u16>(), next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_config_address_encodes_bdf_and_register() {
        assert_eq!(pci_config_address(0, 1, 0, 0x10), 0x8000_0810);
        assert_eq!(pci_config_address(2, 3, 4, 0x13), 0x8002_1C10);
    }

    #[test]
    fn used_ring_is_page_aligned_after_available_ring() {
        assert_eq!(align_up(4 + QUEUE_SIZE * 16 + 4 + QUEUE_SIZE * 2, 4096), 4096);
    }
}