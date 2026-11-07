//! A bounded, coalescing kernel heap for the freestanding x86_64 kernel.
//!
//! The first heap is a 1 MiB statically reserved BSS arena. It is initialized
//! only after the kernel-owned RW/NX page map is active. Allocation is first-fit,
//! aligned, split on allocation, and coalesced on deallocation.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::mem::{align_of, size_of};
use core::ops::{Deref, DerefMut};
use core::ptr::{addr_of_mut, null_mut};
use core::sync::atomic::{AtomicBool, Ordering};

pub const HEAP_SIZE: usize = 1024 * 1024;
const BLOCK_ALIGNMENT: usize = align_of::<Block>();

#[repr(C, align(16))]
struct Block {
    span: usize,
    next: *mut Block,
}

#[repr(C)]
struct AllocationHeader {
    block: *mut Block,
}

struct HeapState {
    free_head: *mut Block,
    initialized: bool,
}

impl HeapState {
    const fn new() -> Self {
        Self { free_head: null_mut(), initialized: false }
    }

    unsafe fn initialize(&mut self, start: *mut u8, length: usize) -> Result<(), HeapInitError> {
        if self.initialized {
            return Err(HeapInitError::AlreadyInitialized);
        }
        let raw_start = start as usize;
        let raw_end = raw_start.checked_add(length).ok_or(HeapInitError::InvalidRegion)?;
        let aligned_start = align_up(raw_start, BLOCK_ALIGNMENT).ok_or(HeapInitError::InvalidRegion)?;
        if raw_end <= aligned_start || raw_end - aligned_start < size_of::<Block>() + size_of::<AllocationHeader>() + 1 {
            return Err(HeapInitError::InvalidRegion);
        }
        let first = aligned_start as *mut Block;
        // SAFETY: The caller provides a writable, exclusively owned heap arena.
        unsafe { first.write(Block { span: raw_end - aligned_start, next: null_mut() }); }
        self.free_head = first;
        self.initialized = true;
        Ok(())
    }

    unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
        if !self.initialized {
            return null_mut();
        }
        let requested = layout.size().max(1);
        let alignment = layout.align().max(BLOCK_ALIGNMENT);
        let mut link = addr_of_mut!(self.free_head);
        // SAFETY: The free list contains only valid block headers in the heap arena.
        unsafe {
            while !(*link).is_null() {
                let block = *link;
                let block_start = block as usize;
                let block_end = match block_start.checked_add((*block).span) {
                    Some(end) => end,
                    None => return null_mut(),
                };
                let minimum_payload = match block_start
                    .checked_add(size_of::<Block>())
                    .and_then(|v| v.checked_add(size_of::<AllocationHeader>())) {
                    Some(value) => value,
                    None => return null_mut(),
                };
                let payload = match align_up(minimum_payload, alignment) {
                    Some(value) => value,
                    None => return null_mut(),
                };
                let payload_end = match payload.checked_add(requested) {
                    Some(value) => value,
                    None => return null_mut(),
                };
                let used_end = match align_up(payload_end, BLOCK_ALIGNMENT) {
                    Some(value) => value,
                    None => return null_mut(),
                };

                if used_end <= block_end {
                    let next = (*block).next;
                    let suffix_span = block_end - used_end;
                    if suffix_span >= size_of::<Block>() + 1 {
                        let suffix = used_end as *mut Block;
                        suffix.write(Block { span: suffix_span, next });
                        *link = suffix;
                        (*block).span = used_end - block_start;
                    } else {
                        *link = next;
                        (*block).span = block_end - block_start;
                    }
                    (*block).next = null_mut();
                    let header = (payload - size_of::<AllocationHeader>()) as *mut AllocationHeader;
                    header.write_unaligned(AllocationHeader { block });
                    return payload as *mut u8;
                }
                link = addr_of_mut!((*block).next);
            }
        }
        null_mut()
    }

    unsafe fn deallocate(&mut self, pointer: *mut u8) {
        if pointer.is_null() || !self.initialized {
            return;
        }
        let header_address = (pointer as usize).saturating_sub(size_of::<AllocationHeader>());
        // SAFETY: GlobalAlloc's caller must pass the original live allocation pointer.
        let block = unsafe { (header_address as *const AllocationHeader).read_unaligned().block };
        if block.is_null() {
            return;
        }
        // SAFETY: The allocation header points to the block reserved by allocate().
        unsafe { self.insert_free_block(block); }
    }

    unsafe fn insert_free_block(&mut self, block: *mut Block) {
        let mut link = addr_of_mut!(self.free_head);
        // SAFETY: The block was allocated from this heap and the free list is serialized.
        unsafe {
            while !(*link).is_null() && (*link as usize) < block as usize {
                link = addr_of_mut!((*(*link)).next);
            }
            (*block).next = *link;
            *link = block;

            let mut current = self.free_head;
            while !current.is_null() {
                let next = (*current).next;
                if !next.is_null()
                    && (current as usize).checked_add((*current).span) == Some(next as usize)
                {
                    (*current).span += (*next).span;
                    (*current).next = (*next).next;
                } else {
                    current = next;
                }
            }
        }
    }

    fn free_bytes(&self) -> usize {
        let mut total = 0usize;
        let mut current = self.free_head;
        // SAFETY: The free list is valid while the heap lock is held.
        unsafe {
            while !current.is_null() {
                total = total.saturating_add((*current).span.saturating_sub(size_of::<Block>()));
                current = (*current).next;
            }
        }
        total
    }
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value.checked_add(alignment - 1).map(|v| v & !(alignment - 1))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeapInitError {
    AlreadyInitialized,
    InvalidRegion,
}

#[repr(align(4096))]
struct HeapStorage([u8; HEAP_SIZE]);

static mut HEAP_STORAGE: HeapStorage = HeapStorage([0; HEAP_SIZE]);

struct KernelHeap {
    locked: AtomicBool,
    state: UnsafeCell<HeapState>,
}

// SAFETY: All HeapState access is serialized by `locked`; the arena is static and never moved.
unsafe impl Sync for KernelHeap {}

impl KernelHeap {
    const fn new() -> Self {
        Self { locked: AtomicBool::new(false), state: UnsafeCell::new(HeapState::new()) }
    }

    fn lock(&self) -> HeapGuard<'_> {
        let flags: usize;
        // SAFETY: Save the prior interrupt-enable state before disabling local IRQs.
        unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) flags); }
        while self.locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        HeapGuard { heap: self, restore_interrupts: flags & (1 << 9) != 0 }
    }

    fn initialize(&self) -> Result<(), HeapInitError> {
        let mut state = self.lock();
        // SAFETY: HEAP_STORAGE is a unique static arena and initialization is serialized.
        let start = unsafe { addr_of_mut!(HEAP_STORAGE.0).cast::<u8>() };
        unsafe { state.initialize(start, HEAP_SIZE) }
    }

    fn free_bytes(&self) -> usize {
        self.lock().free_bytes()
    }

    fn is_initialized(&self) -> bool {
        self.lock().initialized
    }
}

struct HeapGuard<'a> {
    heap: &'a KernelHeap,
    restore_interrupts: bool,
}

impl Deref for HeapGuard<'_> {
    type Target = HeapState;
    fn deref(&self) -> &HeapState {
        // SAFETY: The guard exclusively owns the allocator lock.
        unsafe { &*self.heap.state.get() }
    }
}

impl DerefMut for HeapGuard<'_> {
    fn deref_mut(&mut self) -> &mut HeapState {
        // SAFETY: The guard exclusively owns the allocator lock.
        unsafe { &mut *self.heap.state.get() }
    }
}

impl Drop for HeapGuard<'_> {
    fn drop(&mut self) {
        self.heap.locked.store(false, Ordering::Release);
        if self.restore_interrupts {
            // SAFETY: Restore IF only if it was set when the lock was acquired.
            unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
        }
    }
}

#[cfg_attr(not(test), global_allocator)]
static GLOBAL_ALLOCATOR: KernelHeap = KernelHeap::new();

/// Initialize the global allocator. Call once after the kernel RW/NX map is active.
pub fn initialize() -> Result<(), HeapInitError> {
    GLOBAL_ALLOCATOR.initialize()
}

/// Whether the kernel heap has been initialized.
pub fn is_initialized() -> bool {
    GLOBAL_ALLOCATOR.is_initialized()
}

/// Number of bytes currently available in free blocks.
pub fn free_bytes() -> usize {
    GLOBAL_ALLOCATOR.free_bytes()
}

unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut state = self.lock();
        // SAFETY: Allocation is serialized and layout is supplied by GlobalAlloc's caller.
        unsafe { state.allocate(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _layout: Layout) {
        let mut state = self.lock();
        // SAFETY: GlobalAlloc's caller must pass a pointer returned by this allocator.
        unsafe { state.deallocate(pointer); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(align(4096))]
    struct TestStorage([u8; 16 * 1024]);

    #[test]
    fn allocator_honors_alignment_and_reuses_freed_blocks() {
        let mut storage = TestStorage([0; 16 * 1024]);
        let mut state = HeapState::new();
        // SAFETY: TestStorage is an aligned writable arena owned by this test.
        unsafe { state.initialize(storage.0.as_mut_ptr(), storage.0.len()).unwrap(); }
        let initial_free = state.free_bytes();
        let layout = Layout::from_size_align(333, 256).unwrap();
        // SAFETY: The test arena is initialized and the layout is valid.
        let first = unsafe { state.allocate(layout) };
        assert!(!first.is_null());
        assert_eq!(first as usize % 256, 0);
        // SAFETY: The allocation is live and large enough for the byte write.
        unsafe { first.write(0xA5); }
        assert_eq!(unsafe { first.read() }, 0xA5);
        // SAFETY: Return the exact pointer allocated above.
        unsafe { state.deallocate(first); }
        assert!(state.free_bytes() >= initial_free);
        let second = unsafe { state.allocate(layout) };
        assert_eq!(second, first);
    }

    #[test]
    fn allocator_splits_and_coalesces_adjacent_free_blocks() {
        let mut storage = TestStorage([0; 16 * 1024]);
        let mut state = HeapState::new();
        // SAFETY: TestStorage is an aligned writable arena owned by this test.
        unsafe { state.initialize(storage.0.as_mut_ptr(), storage.0.len()).unwrap(); }
        let initial_free = state.free_bytes();
        let small = Layout::from_size_align(128, 16).unwrap();
        let large = Layout::from_size_align(4096, 16).unwrap();
        let a = unsafe { state.allocate(small) };
        let b = unsafe { state.allocate(small) };
        assert!(!a.is_null() && !b.is_null() && a != b);
        unsafe { state.deallocate(a); }
        unsafe { state.deallocate(b); }
        assert!(state.free_bytes() >= initial_free);
        let whole = unsafe { state.allocate(large) };
        assert!(!whole.is_null());
    }

    #[test]
    fn allocator_rejects_invalid_arena_and_exhaustion() {
        let mut storage = TestStorage([0; 16 * 1024]);
        let mut state = HeapState::new();
        // SAFETY: The short range is inside the test-owned storage.
        assert_eq!(unsafe { state.initialize(storage.0.as_mut_ptr(), 8) }, Err(HeapInitError::InvalidRegion));
        // SAFETY: The full test arena is valid and exclusively owned.
        unsafe { state.initialize(storage.0.as_mut_ptr(), storage.0.len()).unwrap(); }
        let huge = Layout::from_size_align(32 * 1024, 16).unwrap();
        assert!(unsafe { state.allocate(huge) }.is_null());
    }
}
