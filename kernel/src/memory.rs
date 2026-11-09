//! UEFI memory-map parsing and a bounded physical-frame allocator.
//!
//! Only EfiConventionalMemory (type 7) is initially reclaimable. Loader,
//! runtime, MMIO and ACPI regions are deliberately excluded.

pub const PAGE_SIZE: u64 = 4096;
pub const UEFI_MEMORY_DESCRIPTOR_MIN_SIZE: usize = 40;
pub const UEFI_MEMORY_CONVENTIONAL: u32 = 7;
pub const MAX_MEMORY_RANGES: usize = 128;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MemoryRange {
    start: u64,
    end: u64, // exclusive
}

impl MemoryRange {
    const EMPTY: Self = Self { start: 0, end: 0 };

    const fn contains(&self, address: u64) -> bool {
        address >= self.start && address < self.end
    }

    const fn overlaps(&self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryMapError {
    InvalidDescriptorSize,
    InvalidMapLength,
    TooManyMemoryRanges,
    InvalidRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameFreeError {
    UnalignedAddress,
    AddressNotManaged,
    AlreadyFree,
    TooManyFreeRanges,
}

/// A no-allocation physical frame allocator backed by a fixed-size range table.
///
/// The allocator returns physical addresses. Dereferencing those addresses is
/// only valid while the current firmware-provided identity mapping remains active.
pub struct PhysicalFrameAllocator {
    managed: [MemoryRange; MAX_MEMORY_RANGES],
    managed_count: usize,
    free: [MemoryRange; MAX_MEMORY_RANGES],
    free_count: usize,
}

impl PhysicalFrameAllocator {
    pub const fn empty() -> Self {
        Self {
            managed: [MemoryRange::EMPTY; MAX_MEMORY_RANGES],
            managed_count: 0,
            free: [MemoryRange::EMPTY; MAX_MEMORY_RANGES],
            free_count: 0,
        }
    }

    /// Parse UEFI descriptors from a byte slice. The descriptor layout follows
    /// the x86_64 UEFI ABI: Type@0, PhysicalStart@8, NumberOfPages@24.
    pub fn from_uefi_memory_map(
        bytes: &[u8],
        descriptor_size: usize,
    ) -> Result<Self, MemoryMapError> {
        if descriptor_size < UEFI_MEMORY_DESCRIPTOR_MIN_SIZE {
            return Err(MemoryMapError::InvalidDescriptorSize);
        }
        if bytes.len() % descriptor_size != 0 {
            return Err(MemoryMapError::InvalidMapLength);
        }

        let mut allocator = Self::empty();
        for descriptor in bytes.chunks_exact(descriptor_size) {
            let memory_type = read_u32(descriptor, 0);
            if memory_type != Some(UEFI_MEMORY_CONVENTIONAL) {
                continue;
            }

            let start = read_u64(descriptor, 8).ok_or(MemoryMapError::InvalidRange)?;
            let pages = read_u64(descriptor, 24).ok_or(MemoryMapError::InvalidRange)?;
            if pages == 0 {
                continue;
            }
            if start % PAGE_SIZE != 0 {
                return Err(MemoryMapError::InvalidRange);
            }

            let length = pages.checked_mul(PAGE_SIZE).ok_or(MemoryMapError::InvalidRange)?;
            let end = start.checked_add(length).ok_or(MemoryMapError::InvalidRange)?;
            let range = MemoryRange { start, end };

            insert_range(&mut allocator.managed, &mut allocator.managed_count, range)
                .map_err(|_| MemoryMapError::TooManyMemoryRanges)?;
            insert_range(&mut allocator.free, &mut allocator.free_count, range)
                .map_err(|_| MemoryMapError::TooManyMemoryRanges)?;
        }

        Ok(allocator)
    }

    /// Allocate one 4 KiB physical frame, returning its physical address.
    pub fn allocate_frame(&mut self) -> Option<u64> {
        if self.free_count == 0 {
            return None;
        }

        let address = self.free[0].start;
        self.free[0].start += PAGE_SIZE;
        if self.free[0].start == self.free[0].end {
            remove_range(&mut self.free, &mut self.free_count, 0);
        }
        Some(address)
    }

    /// Allocate a physically contiguous run of page-aligned frames.
    pub fn allocate_contiguous_frames(&mut self, count: usize) -> Option<u64> {
        if count == 0 {
            return None;
        }
        let bytes = u64::try_from(count).ok()?.checked_mul(PAGE_SIZE)?;
        for index in 0..self.free_count {
            let range = self.free[index];
            if range.end - range.start < bytes {
                continue;
            }
            let address = range.start;
            self.free[index].start = address.checked_add(bytes)?;
            if self.free[index].start == self.free[index].end {
                remove_range(&mut self.free, &mut self.free_count, index);
            }
            return Some(address);
        }
        None
    }

    /// Allocate one free 4 KiB frame strictly below `limit`, if any exists.
    ///
    /// Preference ordering matches `allocate_frame`: the lowest free page
    /// wins. When the only candidate span crosses `limit`, the highest whole
    /// page below the limit is cut from its top so the span can never hand
    /// out frames at or above the limit. Page 0 is never returned; the null
    /// page stays permanently reserved.
    pub fn allocate_frame_below(&mut self, limit: u64) -> Option<u64> {
        if limit <= PAGE_SIZE {
            return None;
        }
        // Highest page-aligned address that still fits a whole page below
        // the limit; used when a span must be cut at the limit.
        let split_limit = (limit - PAGE_SIZE) & !(PAGE_SIZE - 1);
        for index in 0..self.free_count {
            let range = self.free[index];
            if range.start >= limit {
                break; // sorted ascending: later spans start at or beyond it
            }
            // Page 0 is reserved, so candidates of a page-0 span begin above it.
            let first = if range.start == 0 { PAGE_SIZE } else { range.start };
            if first + PAGE_SIZE > limit {
                continue; // no whole page fits below the limit in this span
            }
            let address = if range.end <= limit {
                first
            } else {
                split_limit.max(first)
            };
            if address == range.start || (range.start == 0 && address == PAGE_SIZE) {
                // Head-advance: the taken page sits at the span bottom. This
                // also permanently retires page 0 from a page-0 span.
                self.free[index].start = address + PAGE_SIZE;
                if self.free[index].start == self.free[index].end {
                    remove_range(&mut self.free, &mut self.free_count, index);
                }
            } else {
                // Top-cut: only [range.start, address) remains free below.
                self.free[index].end = address;
            }
            return Some(address);
        }
        None
    }

    /// Return a previously allocated frame to the free list.
    pub fn free_frame(&mut self, address: u64) -> Result<(), FrameFreeError> {
        if address % PAGE_SIZE != 0 {
            return Err(FrameFreeError::UnalignedAddress);
        }

        let frame = MemoryRange {
            start: address,
            end: address.checked_add(PAGE_SIZE).ok_or(FrameFreeError::AddressNotManaged)?,
        };
        if !self.managed[..self.managed_count].iter().any(|range| {
            range.contains(address) && frame.end <= range.end
        }) {
            return Err(FrameFreeError::AddressNotManaged);
        }
        if self.free[..self.free_count].iter().any(|range| range.overlaps(frame)) {
            return Err(FrameFreeError::AlreadyFree);
        }

        insert_range(&mut self.free, &mut self.free_count, frame)
            .map_err(|_| FrameFreeError::TooManyFreeRanges)
    }

    pub const fn free_range_count(&self) -> usize {
        self.free_count
    }

    pub fn free_pages(&self) -> u64 {
        self.free[..self.free_count]
            .iter()
            .map(|range| (range.end - range.start) / PAGE_SIZE)
            .sum()
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let raw: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(raw))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let raw: [u8; 8] = bytes.get(offset..offset + 8)?.try_into().ok()?;
    Some(u64::from_le_bytes(raw))
}

fn insert_range(
    ranges: &mut [MemoryRange; MAX_MEMORY_RANGES],
    count: &mut usize,
    range: MemoryRange,
) -> Result<(), ()> {
    if range.start >= range.end {
        return Err(());
    }

    let mut index = 0;
    while index < *count && ranges[index].start < range.start {
        index += 1;
    }
    if *count == MAX_MEMORY_RANGES {
        return Err(());
    }
    for move_index in (index..*count).rev() {
        ranges[move_index + 1] = ranges[move_index];
    }
    ranges[index] = range;
    *count += 1;

    let mut current = 0;
    while current + 1 < *count {
        if ranges[current].end >= ranges[current + 1].start {
            if ranges[current + 1].end > ranges[current].end {
                ranges[current].end = ranges[current + 1].end;
            }
            remove_range(ranges, count, current + 1);
        } else {
            current += 1;
        }
    }
    Ok(())
}

fn remove_range(
    ranges: &mut [MemoryRange; MAX_MEMORY_RANGES],
    count: &mut usize,
    index: usize,
) {
    for move_index in index..(*count - 1) {
        ranges[move_index] = ranges[move_index + 1];
    }
    *count -= 1;
    ranges[*count] = MemoryRange::EMPTY;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(kind: u32, start: u64, pages: u64) -> [u8; 40] {
        let mut bytes = [0u8; 40];
        bytes[0..4].copy_from_slice(&kind.to_le_bytes());
        bytes[8..16].copy_from_slice(&start.to_le_bytes());
        bytes[24..32].copy_from_slice(&pages.to_le_bytes());
        bytes
    }

    #[test]
    fn only_conventional_memory_is_reclaimable() {
        let mut map = [0u8; 80];
        map[..40].copy_from_slice(&descriptor(7, 0x100000, 4));
        map[40..].copy_from_slice(&descriptor(6, 0x200000, 8));
        let allocator = PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).unwrap();
        assert_eq!(allocator.free_pages(), 4);
    }

    #[test]
    fn rejects_descriptors_smaller_than_the_uefi_abi() {
        assert_eq!(
            PhysicalFrameAllocator::from_uefi_memory_map(&[0; 32], 32).err(),
            Some(MemoryMapError::InvalidDescriptorSize)
        );
    }

    #[test]
    fn rejects_a_truncated_memory_map() {
        assert_eq!(
            PhysicalFrameAllocator::from_uefi_memory_map(&[0; 41], 40).err(),
            Some(MemoryMapError::InvalidMapLength)
        );
    }

    #[test]
    fn rejects_unaligned_conventional_memory() {
        let map = descriptor(7, 0x100001, 1);
        assert_eq!(
            PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).err(),
            Some(MemoryMapError::InvalidRange)
        );
    }

    #[test]
    fn allocates_and_reuses_a_returned_frame() {
        let map = descriptor(7, 0x400000, 3);
        let mut allocator = PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).unwrap();
        let first = allocator.allocate_frame().unwrap();
        assert_eq!(first, 0x400000);
        assert_eq!(allocator.free_pages(), 2);
        allocator.free_frame(first).unwrap();
        assert_eq!(allocator.allocate_frame(), Some(first));
    }

    #[test]
    fn allocates_only_contiguous_page_runs() {
        let mut map = [0u8; 80];
        map[..40].copy_from_slice(&descriptor(7, 0x400000, 1));
        map[40..].copy_from_slice(&descriptor(7, 0x500000, 3));
        let mut allocator = PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).unwrap();
        assert_eq!(allocator.allocate_contiguous_frames(2), Some(0x500000));
        assert_eq!(allocator.free_pages(), 2);
        assert_eq!(allocator.allocate_contiguous_frames(3), None);
        assert_eq!(allocator.allocate_contiguous_frames(0), None);
    }

    #[test]
    fn rejects_double_free_and_unmanaged_addresses() {
        let map = descriptor(7, 0x400000, 2);
        let mut allocator = PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).unwrap();
        assert_eq!(allocator.free_frame(0x400000), Err(FrameFreeError::AlreadyFree));
        assert_eq!(allocator.free_frame(0x900000), Err(FrameFreeError::AddressNotManaged));
    }

    #[test]
    fn rejects_unaligned_free_addresses() {
        let map = descriptor(7, 0x400000, 1);
        let mut allocator = PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).unwrap();
        assert_eq!(allocator.free_frame(0x400001), Err(FrameFreeError::UnalignedAddress));
    }

    #[test]
    fn coalesces_adjacent_conventional_ranges() {
        let mut map = [0u8; 80];
        map[..40].copy_from_slice(&descriptor(7, 0x400000, 2));
        map[40..].copy_from_slice(&descriptor(7, 0x402000, 2));
        let allocator = PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).unwrap();
        assert_eq!(allocator.free_range_count(), 1);
        assert_eq!(allocator.free_pages(), 4);
    }

    #[test]
    fn allocate_frame_below_picks_the_lowest_free_frame() {
        let map = descriptor(7, 0x400000, 4);
        let mut allocator = PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).unwrap();
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), Some(0x400000));
        assert_eq!(allocator.free_pages(), 3);
        // The head advances one page at a time while frames remain.
        assert_eq!(allocator.allocate_frame(), Some(0x401000));
    }

    #[test]
    fn allocate_frame_below_cuts_a_span_that_crosses_the_limit() {
        // [0x400000, 0x480000) straddles the 4.25 MiB limit.
        let map = descriptor(7, 0x400000, 128);
        let mut allocator = PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).unwrap();
        assert_eq!(allocator.allocate_frame_below(0x440000), Some(0x43F000));
        assert_eq!(allocator.free_pages(), 63);
        // The untouched bottom stays available and can never re-hand 0x43F000.
        assert_eq!(allocator.allocate_frame(), Some(0x400000));
        assert_eq!(allocator.allocate_frame_below(0x440000), Some(0x401000));
        assert_eq!(allocator.allocate_frame_below(0x440000), Some(0x402000));
    }

    #[test]
    fn allocate_frame_below_never_returns_the_null_page() {
        let map = descriptor(7, 0x0, 3);
        let mut allocator = PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).unwrap();
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), Some(0x1000));
        assert_eq!(allocator.free_pages(), 1);
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), Some(0x2000));
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), None);
    }

    #[test]
    fn allocate_frame_below_rejects_unplaceable_requests() {
        let mut map = [0u8; 120];
        map[..40].copy_from_slice(&descriptor(7, 0x300000, 2));
        map[40..80].copy_from_slice(&descriptor(7, 0x900000, 4));
        map[80..].copy_from_slice(&descriptor(7, 0xFFF000, 1));
        let mut allocator = PhysicalFrameAllocator::from_uefi_memory_map(&map, 40).unwrap();
        // No whole page fits below the trivial limits.
        assert_eq!(allocator.allocate_frame_below(0), None);
        assert_eq!(allocator.allocate_frame_below(0x1000), None);
        assert_eq!(allocator.allocate_frame_below(0x3000), None);
        // Both lower spans start beyond the 2 MiB limit.
        assert_eq!(allocator.allocate_frame_below(0x200000), None);
        // The spans are still handed out lowest-first up to the limit.
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), Some(0x300000));
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), Some(0x301000));
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), Some(0x900000));
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), Some(0x901000));
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), Some(0x902000));
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), Some(0x903000));
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), Some(0xFFF000));
        assert_eq!(allocator.allocate_frame_below(0x0100_0000), None);
    }
}
