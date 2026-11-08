//! Root filesystem bring-up from the SYSFS image served by virtio-blk.
//!
//! The disk is optional: a boot without a virtio-blk device (or with an image
//! that does not parse) keeps running and reports a diagnostic instead of
//! stopping the kernel.

use alloc::{boxed::Box, vec::Vec};

use crate::{
    memory::PhysicalFrameAllocator,
    pci,
    simple_fs::SimpleFs,
    sync::SpinLock,
    vfs::Vfs,
    virtio_blk::VirtioBlk,
};

/// Upper bound on the boot-time SYSFS image so a bad disk cannot exhaust the heap.
pub const MAX_ROOT_IMAGE_BYTES: usize = 64 * 1024;
const SECTOR_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootFsError {
    DeviceNotFound,
    DeviceInitialization,
    DiskRead,
    InvalidImage,
    Allocation,
    Mount,
}

static ROOT: SpinLock<Option<Vfs<'static>>> = SpinLock::new(None);

/// Read, validate and mount the SYSFS image exposed by virtio-blk.
///
/// Returns the number of sectors that were read. On success the mounted VFS is
/// published for `with_root`.
pub fn mount(allocator: &mut PhysicalFrameAllocator) -> Result<u64, RootFsError> {
    // SAFETY: PCI enumeration only reads configuration space of the boot machine.
    let device = unsafe { pci::discover_virtio_blk() }.map_err(|_| RootFsError::DeviceNotFound)?;
    // SAFETY: the device was just discovered and no other code owns it.
    let mut block = unsafe { VirtioBlk::configure(device, allocator) }
        .map_err(|_| RootFsError::DeviceInitialization)?;

    let mut header = [0u8; SECTOR_BYTES];
    // SAFETY: sector 0 is always addressable and `header` is a 512-byte buffer.
    unsafe { block.read_sector(0, &mut header, allocator) }
        .map_err(|_| RootFsError::DiskRead)?;

    let image_size =
        SimpleFs::required_image_size(&header).map_err(|_| RootFsError::InvalidImage)?;
    if !valid_image_size(image_size) {
        return Err(RootFsError::InvalidImage);
    }

    let mut image = Vec::new();
    image
        .try_reserve_exact(image_size)
        .map_err(|_| RootFsError::Allocation)?;
    image.resize(image_size, 0);
    image[..SECTOR_BYTES].copy_from_slice(&header);
    for sector in 1..image_size / SECTOR_BYTES {
        let mut contents = [0u8; SECTOR_BYTES];
        // SAFETY: `sector` is inside the image bounds validated above.
        unsafe { block.read_sector(sector as u64, &mut contents, allocator) }
            .map_err(|_| RootFsError::DiskRead)?;
        let offset = sector * SECTOR_BYTES;
        image[offset..offset + SECTOR_BYTES].copy_from_slice(&contents);
    }

    // The root filesystem lives for the whole kernel lifetime, so the image is
    // intentionally leaked to give the VFS a `'static` borrow.
    let image: &'static [u8] = Box::leak(image.into_boxed_slice());
    let filesystem = SimpleFs::parse(image).map_err(|_| RootFsError::InvalidImage)?;
    let mut vfs = Vfs::new();
    vfs.mount(filesystem).map_err(|_| RootFsError::Mount)?;

    let mut root = ROOT.lock();
    if root.is_some() {
        return Err(RootFsError::Mount);
    }
    *root = Some(vfs);
    Ok((image_size / SECTOR_BYTES) as u64)
}

fn valid_image_size(image_size: usize) -> bool {
    (SECTOR_BYTES..=MAX_ROOT_IMAGE_BYTES).contains(&image_size)
        && image_size % SECTOR_BYTES == 0
}

/// Run `f` on the mounted root filesystem, or return `None` when none is mounted.
pub fn with_root<R>(f: impl FnOnce(&mut Vfs<'static>) -> R) -> Option<R> {
    let mut root = ROOT.lock();
    root.as_mut().map(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_whole_sectors_within_the_budget() {
        assert!(valid_image_size(3 * SECTOR_BYTES));
        assert!(valid_image_size(MAX_ROOT_IMAGE_BYTES));
        assert!(!valid_image_size(0));
        assert!(!valid_image_size(SECTOR_BYTES - 1));
        assert!(!valid_image_size(SECTOR_BYTES + 1));
        assert!(!valid_image_size(MAX_ROOT_IMAGE_BYTES + SECTOR_BYTES));
    }

    #[test]
    fn without_a_mount_the_root_is_absent() {
        // The static starts empty and only `mount` can fill it; the host test
        // binary never calls `mount`, so the accessor must not invent a VFS.
        assert!(with_root(|_| ()).is_none());
    }
}
