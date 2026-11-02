//! Kernel-owned x86_64 four-level page tables with a low-half identity map.
//!
//! K2.3 deliberately retains identity virtual addresses for the boot image,
//! stack, descriptor tables, UEFI handoff structures and framebuffer. It maps
//! the first 512 GiB using 2 MiB pages and does not yet enable NX/W^X policy.

use crate::memory::{PhysicalFrameAllocator, PAGE_SIZE};

pub const HUGE_PAGE_SIZE: u64 = 2 * 1024 * 1024;
pub const IDENTITY_MAP_BYTES: u64 = 512 * 1024 * 1024 * 1024;
const ENTRIES_PER_TABLE: usize = 512;
const PAGE_PRESENT: u64 = 1;
const PAGE_WRITABLE: u64 = 1 << 1;
const PAGE_SIZE_FLAG: u64 = 1 << 7;
const HUGE_ADDRESS_MASK: u64 = 0x000F_FFFF_FFE0_0000;
const PAGE_TABLE_FRAME_COUNT: usize = 2 + ENTRIES_PER_TABLE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagingError {
    OutOfFrames,
    FrameOutsideIdentityWindow,
    InvalidFrameAlignment,
}

/// Allocate and initialize a PML4, one PDPT and 512 page directories.
///
/// # Safety
/// The current UEFI page tables must identity-map conventional physical memory
/// returned by allocator. The caller must keep interrupts disabled and must
/// not enable hardware IRQs until their handlers and controllers are ready.
pub unsafe fn build_identity_map(
    allocator: &mut PhysicalFrameAllocator,
) -> Result<u64, PagingError> {
    let mut frames = [0u64; PAGE_TABLE_FRAME_COUNT];
    for frame in &mut frames {
        *frame = allocator.allocate_frame().ok_or(PagingError::OutOfFrames)?;
        if *frame % PAGE_SIZE != 0 {
            return Err(PagingError::InvalidFrameAlignment);
        }
        if *frame >= IDENTITY_MAP_BYTES {
            return Err(PagingError::FrameOutsideIdentityWindow);
        }
    }

    // SAFETY: The UEFI identity map is required by this function's contract.
    // Every frame was obtained from the physical allocator and is one page long.
    unsafe {
        for frame in &frames {
            core::ptr::write_bytes(*frame as *mut u8, 0, PAGE_SIZE as usize);
        }
    }

    let pml4 = frames[0] as *mut u64;
    let pdpt = frames[1] as *mut u64;

    // PML4[0] maps the lower canonical 512 GiB address window.
    unsafe {
        pml4.write(frames[1] | PAGE_PRESENT | PAGE_WRITABLE);
    }

    for pdpt_index in 0..ENTRIES_PER_TABLE {
        let directory_frame = frames[2 + pdpt_index];
        unsafe {
            pdpt.add(pdpt_index)
                .write(directory_frame | PAGE_PRESENT | PAGE_WRITABLE);
        }

        let directory = directory_frame as *mut u64;
        let one_gib_base = (pdpt_index as u64) * 1024 * 1024 * 1024;
        for directory_index in 0..ENTRIES_PER_TABLE {
            let physical = one_gib_base + (directory_index as u64) * HUGE_PAGE_SIZE;
            let entry = (physical & HUGE_ADDRESS_MASK)
                | PAGE_PRESENT
                | PAGE_WRITABLE
                | PAGE_SIZE_FLAG;
            unsafe {
                directory.add(directory_index).write(entry);
            }
        }
    }

    Ok(frames[0])
}

/// Return the PML4, PDPT, PD and PT indices for a canonical virtual address.
pub const fn page_table_indices(address: u64) -> [usize; 4] {
    [
        ((address >> 39) & 0x1FF) as usize,
        ((address >> 30) & 0x1FF) as usize,
        ((address >> 21) & 0x1FF) as usize,
        ((address >> 12) & 0x1FF) as usize,
    ]
}

/// Check whether an address belongs to the identity-mapped low canonical window.
pub const fn is_identity_mapped(address: u64) -> bool {
    address < IDENTITY_MAP_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_exactly_the_lower_512_gib_canonical_window() {
        assert_eq!(IDENTITY_MAP_BYTES, 1u64 << 39);
        assert!(is_identity_mapped(0));
        assert!(is_identity_mapped(0x7FFF_FFFF));
        assert!(is_identity_mapped(IDENTITY_MAP_BYTES - 1));
        assert!(!is_identity_mapped(IDENTITY_MAP_BYTES));
    }

    #[test]
    fn calculates_page_table_indices_at_each_level() {
        assert_eq!(page_table_indices(0), [0, 0, 0, 0]);
        assert_eq!(page_table_indices(0x1_000), [0, 0, 0, 1]);
        assert_eq!(page_table_indices(HUGE_PAGE_SIZE), [0, 0, 1, 0]);
        assert_eq!(page_table_indices(1024 * 1024 * 1024), [0, 1, 0, 0]);
        assert_eq!(page_table_indices(1u64 << 39), [1, 0, 0, 0]);
    }

    #[test]
    fn reserves_two_root_tables_and_all_page_directories() {
        assert_eq!(PAGE_TABLE_FRAME_COUNT, 514);
        assert_eq!(PAGE_TABLE_FRAME_COUNT as u64 * PAGE_SIZE, 2_105_344);
    }

    #[test]
    fn huge_page_entries_use_present_writable_and_page_size_flags() {
        let entry = (0x1234_0000 & HUGE_ADDRESS_MASK)
            | PAGE_PRESENT
            | PAGE_WRITABLE
            | PAGE_SIZE_FLAG;
        assert_eq!(entry & 0xFFF, 0x83);
        assert_eq!(entry & HUGE_ADDRESS_MASK, 0x1234_0000);
    }
}
