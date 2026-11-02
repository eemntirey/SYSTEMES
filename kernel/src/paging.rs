//! Kernel-owned x86_64 paging with NX and per-4-KiB W^X permissions.
use crate::memory::{PhysicalFrameAllocator, PAGE_SIZE};
use core::arch::x86_64::__cpuid;

pub const HUGE_PAGE_SIZE: u64 = 2 * 1024 * 1024;
pub const IDENTITY_MAP_BYTES: u64 = 512 * 1024 * 1024 * 1024;
const ENTRIES: usize = 512;
const PRESENT: u64 = 1;
const WRITABLE: u64 = 1 << 1;
const HUGE: u64 = 1 << 7;
const NX: u64 = 1 << 63;
const ADDRESS_MASK: u64 = 0x000F_FFFF_FFFF_F000;
const HUGE_ADDRESS_MASK: u64 = 0x000F_FFFF_FFE0_0000;
const FRAME_COUNT: usize = 2 + ENTRIES;
const EFER_MSR: u32 = 0xC000_0080;
const EFER_NXE: u64 = 1 << 11;
const CPUID_NX: u32 = 1 << 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagingError {
    OutOfFrames,
    FrameOutsideIdentityWindow,
    InvalidFrameAlignment,
    NxUnsupported,
    InvalidRegion,
    OverlappingRegions,
    WriteExecuteViolation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagePermissions { ReadOnlyExecute, ReadOnlyNoExecute, ReadWriteNoExecute }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryRegion {
    pub start: u64,
    pub end: u64,
    pub permissions: PagePermissions,
}

impl PagePermissions {
    const fn flags(self) -> u64 {
        match self {
            Self::ReadOnlyExecute => PRESENT,
            Self::ReadOnlyNoExecute => PRESENT | NX,
            Self::ReadWriteNoExecute => PRESENT | WRITABLE | NX,
        }
    }
    pub const fn is_write_execute(self) -> bool { false }
}

pub fn enable_nx() -> Result<(), PagingError> {
    // SAFETY: CPUID is available on the x86_64 target.
    let max = unsafe { __cpuid(0x8000_0000) }.eax;
    if max < 0x8000_0001 { return Err(PagingError::NxUnsupported); }
    // SAFETY: Extended CPUID leaf is supported by the preceding check.
    if unsafe { __cpuid(0x8000_0001) }.edx & CPUID_NX == 0 {
        return Err(PagingError::NxUnsupported);
    }
    let low: u32;
    let high: u32;
    // SAFETY: EFER is the architectural extended feature-enable MSR.
    unsafe { core::arch::asm!("rdmsr", in("ecx") EFER_MSR, out("eax") low, out("edx") high, options(nomem, nostack)); }
    let value = (((high as u64) << 32) | low as u64) | EFER_NXE;
    // SAFETY: NX was confirmed by CPUID before setting EFER.NXE.
    unsafe { core::arch::asm!("wrmsr", in("ecx") EFER_MSR, in("eax") value as u32, in("edx") (value >> 32) as u32, options(nomem, nostack)); }
    Ok(())
}

pub unsafe fn build_identity_map(allocator: &mut PhysicalFrameAllocator) -> Result<u64, PagingError> {
    let mut frames = [0u64; FRAME_COUNT];
    for frame in &mut frames {
        *frame = allocator.allocate_frame().ok_or(PagingError::OutOfFrames)?;
        if *frame % PAGE_SIZE != 0 { return Err(PagingError::InvalidFrameAlignment); }
        if *frame >= IDENTITY_MAP_BYTES { return Err(PagingError::FrameOutsideIdentityWindow); }
    }
    // SAFETY: Allocated frames are identity-accessible under the inherited UEFI map.
    unsafe { for frame in &frames { core::ptr::write_bytes(*frame as *mut u8, 0, PAGE_SIZE as usize); } }
    let pml4 = frames[0] as *mut u64;
    let pdpt = frames[1] as *mut u64;
    // SAFETY: Both tables are freshly allocated, zeroed, page-aligned frames.
    unsafe { pml4.write(frames[1] | PRESENT | WRITABLE); }
    for i in 0..ENTRIES {
        let directory_frame = frames[2 + i];
        // SAFETY: Each directory frame belongs to the allocated table set.
        unsafe { pdpt.add(i).write(directory_frame | PRESENT | WRITABLE); }
        let directory = directory_frame as *mut u64;
        let base = i as u64 * 1024 * 1024 * 1024;
        for j in 0..ENTRIES {
            let physical = base + j as u64 * HUGE_PAGE_SIZE;
            // All generic identity mappings are writable and NX by default.
            let entry = (physical & HUGE_ADDRESS_MASK) | PRESENT | WRITABLE | HUGE | NX;
            // SAFETY: The directory has exactly 512 entries.
            unsafe { directory.add(j).write(entry); }
        }
    }
    Ok(frames[0])
}

fn validate_regions(regions: &[MemoryRegion]) -> Result<(), PagingError> {
    for region in regions {
        if region.start >= region.end || region.end > IDENTITY_MAP_BYTES
            || region.start % PAGE_SIZE != 0 || region.end % PAGE_SIZE != 0 {
            return Err(PagingError::InvalidRegion);
        }
        if region.permissions.is_write_execute() { return Err(PagingError::WriteExecuteViolation); }
    }
    for i in 0..regions.len() {
        for j in i + 1..regions.len() {
            if regions[i].start < regions[j].end && regions[j].start < regions[i].end {
                return Err(PagingError::OverlappingRegions);
            }
        }
    }
    Ok(())
}

unsafe fn ensure_4k_table(
    allocator: &mut PhysicalFrameAllocator, root: u64, address: u64
) -> Result<*mut u64, PagingError> {
    if address >= IDENTITY_MAP_BYTES { return Err(PagingError::InvalidRegion); }
    let pml4 = root as *mut u64;
    let pdpt_index = ((address >> 30) & 0x1ff) as usize;
    let pd_index = ((address >> 21) & 0x1ff) as usize;
    // SAFETY: root and all descendants are kernel-owned identity-mapped tables.
    let pdpt_entry = unsafe { pml4.add(0).read() };
    let pdpt = (pdpt_entry & ADDRESS_MASK) as *mut u64;
    let directory_entry = unsafe { pdpt.add(pdpt_index).read() };
    let directory = (directory_entry & ADDRESS_MASK) as *mut u64;
    let pde_ptr = unsafe { directory.add(pd_index) };
    let pde = unsafe { pde_ptr.read() };
    if pde & HUGE != 0 {
        let table_frame = allocator.allocate_frame().ok_or(PagingError::OutOfFrames)?;
        if table_frame % PAGE_SIZE != 0 { return Err(PagingError::InvalidFrameAlignment); }
        // SAFETY: The newly allocated page is reachable through inherited identity mapping.
        unsafe { core::ptr::write_bytes(table_frame as *mut u8, 0, PAGE_SIZE as usize); }
        let table = table_frame as *mut u64;
        let physical_base = pde & HUGE_ADDRESS_MASK;
        for i in 0..ENTRIES {
            let physical = physical_base + i as u64 * PAGE_SIZE;
            // Preserve identity mapping, but make every 4 KiB leaf NX by default.
            unsafe { table.add(i).write(physical | PRESENT | WRITABLE | NX); }
        }
        // Parent must permit both permissions; leaf entries enforce W^X.
        unsafe { pde_ptr.write(table_frame | PRESENT | WRITABLE); }
        Ok(table)
    } else {
        if pde & PRESENT == 0 { return Err(PagingError::InvalidRegion); }
        Ok((pde & ADDRESS_MASK) as *mut u64)
    }
}

pub unsafe fn protect_regions(
    allocator: &mut PhysicalFrameAllocator, root: u64, regions: &[MemoryRegion]
) -> Result<(), PagingError> {
    validate_regions(regions)?;
    for region in regions {
        let mut address = region.start;
        while address < region.end {
            // SAFETY: Caller invokes this while the inherited UEFI identity map is active.
            let table = unsafe { ensure_4k_table(allocator, root, address)? };
            let index = ((address >> 12) & 0x1ff) as usize;
            // SAFETY: The page table has 512 entries and index is in 0..512.
            unsafe { table.add(index).write((address & ADDRESS_MASK) | region.permissions.flags()); }
            address += PAGE_SIZE;
        }
    }
    Ok(())
}

pub const fn page_table_indices(address: u64) -> [usize; 4] {
    [((address >> 39) & 0x1ff) as usize, ((address >> 30) & 0x1ff) as usize,
     ((address >> 21) & 0x1ff) as usize, ((address >> 12) & 0x1ff) as usize]
}
pub const fn is_identity_mapped(address: u64) -> bool { address < IDENTITY_MAP_BYTES }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn lower_512_gib_identity_window_is_exact() {
        assert_eq!(IDENTITY_MAP_BYTES, 1u64 << 39);
        assert!(is_identity_mapped(IDENTITY_MAP_BYTES - 1));
        assert!(!is_identity_mapped(IDENTITY_MAP_BYTES));
    }
    #[test] fn calculates_indices_at_each_level() {
        assert_eq!(page_table_indices(0), [0,0,0,0]);
        assert_eq!(page_table_indices(0x1000), [0,0,0,1]);
        assert_eq!(page_table_indices(HUGE_PAGE_SIZE), [0,0,1,0]);
        assert_eq!(page_table_indices(1u64 << 30), [0,1,0,0]);
    }
    #[test] fn permission_flags_never_create_writable_executable_pages() {
        for p in [PagePermissions::ReadOnlyExecute, PagePermissions::ReadOnlyNoExecute, PagePermissions::ReadWriteNoExecute] {
            let flags = p.flags();
            assert!(!(flags & WRITABLE != 0 && flags & NX == 0));
        }
        assert_eq!(PagePermissions::ReadOnlyExecute.flags() & (WRITABLE | NX), 0);
        assert_eq!(PagePermissions::ReadOnlyNoExecute.flags() & NX, NX);
        assert_eq!(PagePermissions::ReadWriteNoExecute.flags() & (WRITABLE | NX), WRITABLE | NX);
    }
    #[test] fn rejects_unaligned_and_empty_regions() {
        assert_eq!(validate_regions(&[MemoryRegion { start: 1, end: 4096, permissions: PagePermissions::ReadOnlyExecute }]), Err(PagingError::InvalidRegion));
        assert_eq!(validate_regions(&[MemoryRegion { start: 4096, end: 4096, permissions: PagePermissions::ReadOnlyExecute }]), Err(PagingError::InvalidRegion));
    }
    #[test] fn rejects_overlapping_regions() {
        let regions = [
            MemoryRegion { start: 0x200000, end: 0x204000, permissions: PagePermissions::ReadOnlyExecute },
            MemoryRegion { start: 0x203000, end: 0x205000, permissions: PagePermissions::ReadWriteNoExecute },
        ];
        assert_eq!(validate_regions(&regions), Err(PagingError::OverlappingRegions));
    }
    #[test] fn rejects_regions_outside_identity_window() {
        assert_eq!(validate_regions(&[MemoryRegion { start: IDENTITY_MAP_BYTES, end: IDENTITY_MAP_BYTES + PAGE_SIZE, permissions: PagePermissions::ReadOnlyExecute }]), Err(PagingError::InvalidRegion));
    }
}
