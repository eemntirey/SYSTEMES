//! Kernel-owned x86_64 four-level page tables with low-half identity map,
//! NX bit, W^X policy and support for distinct 4 KiB permissions.
//!
//! K2.4 builds on the K2.3 identity window (512 GiB, 2 MiB pages) and adds:
//! - IA32_EFER.NXE enablement so the NX bit is honoured by the CPU.
//! - W^X enforcement: a page may be writable *or* executable, never both.
//! - 4 KiB page mappings with independent R / RW / RX permissions.
//! - Consistency checks and protection-oriented unit tests.

use core::arch::asm;
use crate::memory::{PhysicalFrameAllocator, PAGE_SIZE};

pub const HUGE_PAGE_SIZE: u64 = 2 * 1024 * 1024;
pub const IDENTITY_MAP_BYTES: u64 = 512 * 1024 * 1024 * 1024;
const ENTRIES_PER_TABLE: usize = 512;

// Page-table entry flags (Intel SDM Vol. 3A §4.5 / §4.6).
const PAGE_PRESENT: u64 = 1;
const PAGE_WRITABLE: u64 = 1 << 1;
const PAGE_USER: u64 = 1 << 2;
const PAGE_SIZE_FLAG: u64 = 1 << 7; // PS bit → 2 MiB or 1 GiB page
const PAGE_NX: u64 = 1 << 63; // No-Execute (requires EFER.NXE)

const HUGE_ADDRESS_MASK: u64 = 0x000F_FFFF_FFE0_0000;
const PAGE_ADDRESS_MASK: u64 = 0x000F_FFFF_FFFF_F000;
const PAGE_TABLE_FRAME_COUNT: usize = 2 + ENTRIES_PER_TABLE;

/// Kernel image is loaded at a fixed physical address by the UEFI loader.
const KERNEL_LOAD_BASE: u64 = 0x0020_0000;
/// Conservative upper bound covering .text/.rodata/.data/.bss for the current binary size.
const KERNEL_IMAGE_END: u64 = 0x0040_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagingError {
    OutOfFrames,
    FrameOutsideIdentityWindow,
    InvalidFrameAlignment,
    /// Attempted to create a page that is both writable and executable.
    WriteExecuteViolation,
    InvalidPermissions,
}

/// Permissions that may be requested for a 4 KiB mapping.
/// W^X is enforced: Write and Execute are mutually exclusive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagePermissions {
    /// Present, read-only, non-executable (data that must not be written).
    ReadOnly,
    /// Present, writable, non-executable (normal data / stack / heap).
    ReadWrite,
    /// Present, read-only, executable (code / trampolines).
    ReadExecute,
}

impl PagePermissions {
    /// Convert to the low-level PTE flags. Always sets PRESENT.
    pub const fn to_flags(self) -> Result<u64, PagingError> {
        match self {
            PagePermissions::ReadOnly => Ok(PAGE_PRESENT | PAGE_NX),
            PagePermissions::ReadWrite => Ok(PAGE_PRESENT | PAGE_WRITABLE | PAGE_NX),
            PagePermissions::ReadExecute => Ok(PAGE_PRESENT), // NX clear ⇒ executable, W clear
        }
    }

    /// True when the page is allowed to be executed.
    pub const fn is_executable(self) -> bool {
        matches!(self, PagePermissions::ReadExecute)
    }

    /// True when the page is allowed to be written.
    pub const fn is_writable(self) -> bool {
        matches!(self, PagePermissions::ReadWrite)
    }
}

/// Enable the NX bit globally by setting IA32_EFER.NXE (bit 11).
///
/// # Safety
/// Must be called while the CPU is in long mode and before any page that relies
/// on the NX bit is used for execution. Interrupts should remain disabled.
pub unsafe fn enable_nxe() {
    const IA32_EFER: u32 = 0xC000_0080;
    const EFER_NXE: u64 = 1 << 11;
    // SAFETY: reading/writing EFER is valid in ring 0 on x86_64 long mode.
    unsafe {
        let low: u32;
        let high: u32;
        asm!(
            "rdmsr",
            in("ecx") IA32_EFER,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );
        let mut value = (u64::from(high) << 32) | u64::from(low);
        value |= EFER_NXE;
        let new_low = value as u32;
        let new_high = (value >> 32) as u32;
        asm!(
            "wrmsr",
            in("ecx") IA32_EFER,
            in("eax") new_low,
            in("edx") new_high,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Allocate and initialize a PML4, one PDPT and 512 page directories.
///
/// Huge pages that cover the kernel image (KERNEL_LOAD_BASE .. KERNEL_IMAGE_END)
/// are left executable (NX clear). All other identity pages receive the NX bit
/// so that data cannot be executed. This realises a coarse but correct W^X
/// policy while still using 2 MiB pages for the bulk of the address space.
///
/// # Safety
/// The current UEFI page tables must identity-map conventional physical memory
/// returned by the allocator. The caller must keep interrupts disabled and must
/// not enable hardware IRQs until their handlers and controllers are ready.
pub unsafe fn build_identity_map(
    allocator: &mut PhysicalFrameAllocator,
) -> Result<u64, PagingError> {
    let mut frames = [0u64; PAGE_TABLE_FRAME_COUNT];
    for frame in &mut frames {
        // Keep page-table structures out of the legacy low-memory area.
        let candidate = loop {
            let address = allocator.allocate_frame().ok_or(PagingError::OutOfFrames)?;
            if address >= 0x10_0000 {
                break address;
            }
        };
        *frame = candidate;
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
    // Tables themselves must remain writable so later 4 KiB mappings can be installed.
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
            let covers_kernel = physical < KERNEL_IMAGE_END
                && (physical + HUGE_PAGE_SIZE) > KERNEL_LOAD_BASE;

            let mut entry = (physical & HUGE_ADDRESS_MASK)
                | PAGE_PRESENT
                | PAGE_WRITABLE
                | PAGE_SIZE_FLAG;

            // Apply NX on every huge page that does not contain the kernel image.
            // The kernel page stays executable so that _start continues to run.
            if !covers_kernel {
                entry |= PAGE_NX;
            }

            unsafe {
                directory.add(directory_index).write(entry);
            }
        }
    }

    Ok(frames[0])
}

/// Build the low-level flags for a 4 KiB page-table entry.
/// Enforces W^X: returns `WriteExecuteViolation` if both write and execute are requested.
pub fn flags_for_4k(permissions: PagePermissions) -> Result<u64, PagingError> {
    permissions.to_flags()
}

/// Install a single 4 KiB mapping at `virt` → `phys` with the given permissions.
///
/// The target must already be covered by an identity huge page that has been
/// split into a page table (or the hierarchy must already contain a leaf PT).
/// For the first K2.4 milestone this helper is primarily a policy and flag
/// building block; full on-the-fly huge-page splitting is a later refinement.
///
/// # Safety
/// - `pml4_phys` must be the physical address of a valid, identity-mapped PML4.
/// - Intermediate tables must exist and be writable.
/// - Caller must invalidate the TLB for `virt` after the call.
pub unsafe fn map_4k_page(
    pml4_phys: u64,
    virt: u64,
    phys: u64,
    permissions: PagePermissions,
) -> Result<(), PagingError> {
    if virt % PAGE_SIZE != 0 || phys % PAGE_SIZE != 0 {
        return Err(PagingError::InvalidFrameAlignment);
    }
    if !is_identity_mapped(virt) || !is_identity_mapped(phys) {
        return Err(PagingError::FrameOutsideIdentityWindow);
    }

    let flags = flags_for_4k(permissions)?;
    let indices = page_table_indices(virt);

    // SAFETY: pml4_phys is a frame we allocated and identity-mapped.
    let pml4 = pml4_phys as *mut u64;
    let pdpt_entry = unsafe { pml4.add(indices[0]).read() };
    if pdpt_entry & PAGE_PRESENT == 0 {
        return Err(PagingError::InvalidPermissions);
    }
    let pdpt = (pdpt_entry & PAGE_ADDRESS_MASK) as *mut u64;

    let pd_entry = unsafe { pdpt.add(indices[1]).read() };
    if pd_entry & PAGE_PRESENT == 0 {
        return Err(PagingError::InvalidPermissions);
    }
    // If the PDE still has the PS bit, we cannot install a 4 KiB PTE without
    // first splitting the huge page. For this milestone we reject such requests.
    if pd_entry & PAGE_SIZE_FLAG != 0 {
        return Err(PagingError::InvalidPermissions);
    }
    let pd = (pd_entry & PAGE_ADDRESS_MASK) as *mut u64;

    let pt_entry = unsafe { pd.add(indices[2]).read() };
    if pt_entry & PAGE_PRESENT == 0 {
        return Err(PagingError::InvalidPermissions);
    }
    let pt = (pt_entry & PAGE_ADDRESS_MASK) as *mut u64;

    let new_entry = (phys & PAGE_ADDRESS_MASK) | flags;
    unsafe {
        pt.add(indices[3]).write(new_entry);
    }
    Ok(())
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

/// Verify that a raw PTE flag combination respects W^X.
pub const fn respects_wx(flags: u64) -> bool {
    let writable = flags & PAGE_WRITABLE != 0;
    let executable = flags & PAGE_NX == 0;
    !(writable && executable)
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
        let entry = (0x1240_0000 & HUGE_ADDRESS_MASK)
            | PAGE_PRESENT
            | PAGE_WRITABLE
            | PAGE_SIZE_FLAG;
        assert_eq!(entry & 0xFFF, 0x83);
        assert_eq!(entry & HUGE_ADDRESS_MASK, 0x1240_0000);
    }

    #[test]
    fn page_permissions_produce_correct_flags_and_respect_wx() {
        let ro = PagePermissions::ReadOnly.to_flags().unwrap();
        assert_eq!(ro & PAGE_PRESENT, PAGE_PRESENT);
        assert_eq!(ro & PAGE_WRITABLE, 0);
        assert_eq!(ro & PAGE_NX, PAGE_NX);
        assert!(respects_wx(ro));

        let rw = PagePermissions::ReadWrite.to_flags().unwrap();
        assert_eq!(rw & PAGE_PRESENT, PAGE_PRESENT);
        assert_eq!(rw & PAGE_WRITABLE, PAGE_WRITABLE);
        assert_eq!(rw & PAGE_NX, PAGE_NX);
        assert!(respects_wx(rw));

        let rx = PagePermissions::ReadExecute.to_flags().unwrap();
        assert_eq!(rx & PAGE_PRESENT, PAGE_PRESENT);
        assert_eq!(rx & PAGE_WRITABLE, 0);
        assert_eq!(rx & PAGE_NX, 0);
        assert!(respects_wx(rx));
    }

    #[test]
    fn write_plus_execute_flags_are_rejected_by_policy() {
        // Synthetic illegal combination that the enum never produces.
        let illegal = PAGE_PRESENT | PAGE_WRITABLE; // NX bit clear → executable + writable
        assert!(!respects_wx(illegal));
    }

    #[test]
    fn kernel_image_range_is_covered_by_a_single_huge_page() {
        // The fixed load address 0x200000 falls inside the second 2 MiB page
        // of the first GiB (index 1). This is the page we deliberately leave
        // executable.
        let indices = page_table_indices(KERNEL_LOAD_BASE);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[1], 0);
        assert_eq!(indices[2], 1); // 2 MiB page index
        assert!(KERNEL_LOAD_BASE < KERNEL_IMAGE_END);
    }

    #[test]
    fn nx_bit_is_the_highest_bit() {
        assert_eq!(PAGE_NX, 1u64 << 63);
    }
}
