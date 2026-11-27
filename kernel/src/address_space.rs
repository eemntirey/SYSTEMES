//! Private process page tables sharing the kernel identity mapping.
//!
//! User virtual addresses live in PML4 slot 1 (512 GiB..128 TiB). Every
//! address space copies the kernel PML4 slot 0, so kernel code, stacks and
//! device mappings stay visible after a CR3 switch. User leaves always carry
//! the U/S bit; writable-executable user pages are not representable.

#[cfg(any(feature = "qemu-ring3-test", feature = "qemu-ring3-smp-test"))]
use crate::interrupts::RING3_TEST_MAGIC;
use crate::memory::{PAGE_SIZE, PhysicalFrameAllocator};
use crate::paging::{IDENTITY_MAP_BYTES, page_table_indices};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(any(feature = "qemu-ring3-smp-test", feature = "qemu-ring3-task-test"))]
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};

const PRESENT: u64 = 1;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const HUGE: u64 = 1 << 7;
const NX: u64 = 1 << 63;
const ADDRESS_MASK: u64 = 0x000F_FFFF_FFFF_F000;

/// First user virtual address (PML4 slot 1).
pub const USER_BASE: u64 = 1 << 39;
/// First non-canonical lower-half address (exclusive user limit).
pub const USER_LIMIT: u64 = 1 << 47;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressSpaceError {
    OutOfFrames,
    OutOfMemory,
    ActiveAddressSpace,
    FrameReleaseFailed,
    InvalidFrameAlignment,
    FrameOutsideIdentityWindow,
    KernelRootNotMapped,
    InvalidUserAddress,
    UnalignedAddress,
    AlreadyMapped,
    HugePageUnsupported,
    PermissionMismatch,
    Cr3Mismatch,
    UserReadMismatch,
    UserWriteMismatch,
    DuplicateMappingAccepted,
    InvalidMappingAccepted,
    KernelMappingMissing,
    SmapUnavailable,
    SmepUnavailable,
    SmapDidNotBlock,
    SmepDidNotBlock,
    Ring3TransitionFailed,
    Ring3StackWriteMismatch,
    SyscallTransitionFailed,
    SyscallReturnMismatch,
    SyscallNegativeTestFailed,
    UserProcessExitFailed,
    FrameReclaimMismatch,
    Ring3TaskStateMismatch,
    Ring3TaskExitReasonMismatch,
    Ring3TaskCr3Mismatch,
    Ring3TaskStackMismatch,
    Ring3TaskResumedInUserMode,
    Ring3TaskObserverMissing,
    TlbShootdownTimeout,
}

#[cfg(feature = "qemu-ring3-test")]
const RING3_CODE_ADDRESS: u64 = USER_BASE + 0x4000;
#[cfg(feature = "qemu-ring3-test")]
const RING3_STACK_ADDRESS: u64 = USER_BASE + 0x5000;
#[cfg(feature = "qemu-syscall-test")]
const SYSCALL_CODE_ADDRESS: u64 = USER_BASE + 0x6000;
#[cfg(feature = "qemu-syscall-test")]
const SYSCALL_STACK_ADDRESS: u64 = USER_BASE + 0x7000;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserPagePermissions {
    ReadOnlyExecute,
    ReadOnlyNoExecute,
    ReadWriteNoExecute,
}

impl UserPagePermissions {
    const fn flags(self) -> u64 {
        match self {
            Self::ReadOnlyExecute => PRESENT | USER,
            Self::ReadOnlyNoExecute => PRESENT | USER | NX,
            Self::ReadWriteNoExecute => PRESENT | USER | WRITABLE | NX,
        }
    }
}

pub const fn is_user_address(address: u64) -> bool {
    address >= USER_BASE && address < USER_LIMIT
}

pub const fn cr3_root(value: u64) -> u64 {
    value & ADDRESS_MASK
}

/// A process-owned PML4 and the frames that must be reclaimed on exit.
pub struct AddressSpace {
    root: u64,
    owned_frames: Vec<u64>,
}

impl AddressSpace {
    pub const fn root(&self) -> u64 {
        self.root
    }

    /// Build an ownerless test address space without touching CR3 or memory.
    #[cfg(test)]
    pub(crate) const fn for_test(root: u64) -> Self {
        Self {
            root,
            owned_frames: Vec::new(),
        }
    }

    /// Create a PML4 that shares the kernel identity mapping in slot 0.
    ///
    /// # Safety
    ///
    /// `kernel_root` must be the active kernel PML4 whose tables are reachable
    /// through the current identity mapping.
    pub unsafe fn create(
        allocator: &mut PhysicalFrameAllocator,
        kernel_root: u64,
    ) -> Result<Self, AddressSpaceError> {
        if kernel_root % PAGE_SIZE != 0 {
            return Err(AddressSpaceError::InvalidFrameAlignment);
        }
        if kernel_root >= IDENTITY_MAP_BYTES {
            return Err(AddressSpaceError::FrameOutsideIdentityWindow);
        }
        // SAFETY: kernel_root points to a valid PML4 under the active identity map.
        let kernel_entry_zero = unsafe { (kernel_root as *const u64).read() };
        if kernel_entry_zero & PRESENT == 0 {
            return Err(AddressSpaceError::KernelRootNotMapped);
        }

        let mut owned_frames = Vec::new();
        owned_frames
            .try_reserve_exact(1)
            .map_err(|_| AddressSpaceError::OutOfMemory)?;
        let root = allocator
            .allocate_frame()
            .ok_or(AddressSpaceError::OutOfFrames)?;
        if root % PAGE_SIZE != 0 {
            allocator
                .free_frame(root)
                .map_err(|_| AddressSpaceError::FrameReleaseFailed)?;
            return Err(AddressSpaceError::InvalidFrameAlignment);
        }
        if root >= IDENTITY_MAP_BYTES {
            allocator
                .free_frame(root)
                .map_err(|_| AddressSpaceError::FrameReleaseFailed)?;
            return Err(AddressSpaceError::FrameOutsideIdentityWindow);
        }

        // SAFETY: root is a freshly allocated, page-aligned frame.
        unsafe { core::ptr::write_bytes(root as *mut u8, 0, PAGE_SIZE as usize) };
        // SAFETY: A PML4 has 512 entries; slot 0 shares the kernel identity map.
        unsafe { (root as *mut u64).write(kernel_entry_zero) };

        owned_frames.push(root);
        Ok(Self {
            root,
            owned_frames,
        })
    }

    /// Map one 4 KiB user page with U/S set on every page-table level.
    ///
    /// # Safety
    ///
    /// The allocator must stay valid while the kernel identity mapping is active.
    pub unsafe fn map_user_page(
        &mut self,
        allocator: &mut PhysicalFrameAllocator,
        virtual_address: u64,
        physical_frame: u64,
        permissions: UserPagePermissions,
    ) -> Result<(), AddressSpaceError> {
        let indices = validate_user_mapping(virtual_address, physical_frame)?;
        let pml4 = self.root as *mut u64;

        // SAFETY: The entry pointers belong to tables owned by this address space.
        let pdpt = unsafe { self.next_table(allocator, pml4.add(indices[0]))? };
        // SAFETY: `pdpt` is a valid table allocated above.
        let pd = unsafe { self.next_table(allocator, pdpt.add(indices[1]))? };
        // SAFETY: `pd` is a valid table allocated above.
        let pt = unsafe { self.next_table(allocator, pd.add(indices[2]))? };
        // SAFETY: `pt` is a valid leaf table allocated above.
        let pte = unsafe { pt.add(indices[3]) };
        // SAFETY: The leaf entry is readable and belongs to `pt`.
        if unsafe { pte.read() } & PRESENT != 0 {
            return Err(AddressSpaceError::AlreadyMapped);
        }
        self.record_owned_frame(physical_frame)?;
        // SAFETY: The leaf entry is not present and the frame was validated.
        unsafe { pte.write((physical_frame & ADDRESS_MASK) | permissions.flags()) };
        Ok(())
    }

    fn record_owned_frame(&mut self, frame: u64) -> Result<(), AddressSpaceError> {
        if self.owned_frames.contains(&frame) {
            return Ok(());
        }
        self.owned_frames
            .try_reserve(1)
            .map_err(|_| AddressSpaceError::OutOfMemory)?;
        self.owned_frames.push(frame);
        Ok(())
    }

    /// Return all user mappings and page-table frames to the allocator.
    ///
    /// # Safety
    ///
    /// The caller must switch to a different valid address space before calling.
    pub unsafe fn destroy(
        self,
        allocator: &mut PhysicalFrameAllocator,
    ) -> Result<(), AddressSpaceError> {
        if cr3_root(unsafe { read_cr3() }) == self.root {
            return Err(AddressSpaceError::ActiveAddressSpace);
        }
        for frame in self.owned_frames {
            allocator
                .free_frame(frame)
                .map_err(|_| AddressSpaceError::FrameReleaseFailed)?;
        }
        Ok(())
    }

    unsafe fn next_table(
        &mut self,
        allocator: &mut PhysicalFrameAllocator,
        entry_ptr: *mut u64,
    ) -> Result<*mut u64, AddressSpaceError> {
        // SAFETY: `entry_ptr` belongs to a table owned by this address space.
        let entry = unsafe { entry_ptr.read() };
        if entry & PRESENT != 0 {
            if entry & HUGE != 0 {
                return Err(AddressSpaceError::HugePageUnsupported);
            }
            // SAFETY: `table_pointer` validates the identity-window invariant.
            return unsafe { table_pointer(entry) };
        }

        let frame = allocator
            .allocate_frame()
            .ok_or(AddressSpaceError::OutOfFrames)?;
        if frame % PAGE_SIZE != 0 || frame >= IDENTITY_MAP_BYTES {
            allocator
                .free_frame(frame)
                .map_err(|_| AddressSpaceError::FrameReleaseFailed)?;
            return Err(if frame % PAGE_SIZE != 0 {
                AddressSpaceError::InvalidFrameAlignment
            } else {
                AddressSpaceError::FrameOutsideIdentityWindow
            });
        }
        if let Err(error) = self.record_owned_frame(frame) {
            allocator
                .free_frame(frame)
                .map_err(|_| AddressSpaceError::FrameReleaseFailed)?;
            return Err(error);
        }
        // SAFETY: `frame` is a fresh, aligned identity-accessible frame.
        unsafe { core::ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE as usize) };
        // SAFETY: The parent entry is not present and accepts the new table.
        unsafe { entry_ptr.write(frame | PRESENT | WRITABLE | USER) };
        Ok(frame as *mut u64)
    }

    /// Verify U/S propagation and leaf permissions for one user mapping.
    ///
    /// # Safety
    ///
    /// The address space must be reachable through the active identity mapping.
    pub unsafe fn verify_user_mapping(
        &self,
        virtual_address: u64,
        physical_frame: u64,
        permissions: UserPagePermissions,
    ) -> Result<(), AddressSpaceError> {
        let indices = validate_user_mapping(virtual_address, physical_frame)?;
        let pml4 = self.root as *const u64;
        let user_present = PRESENT | USER;

        // SAFETY: The walk starts from a PML4 owned by this address space.
        let pml4_entry = unsafe { pml4.add(indices[0]).read() };
        if pml4_entry & user_present != user_present {
            return Err(AddressSpaceError::PermissionMismatch);
        }
        let pdpt = unsafe { table_pointer(pml4_entry)? };
        // SAFETY: `pdpt` was validated as an identity-reachable table.
        let pdpt_entry = unsafe { pdpt.add(indices[1]).read() };
        if pdpt_entry & user_present != user_present {
            return Err(AddressSpaceError::PermissionMismatch);
        }
        if pdpt_entry & HUGE != 0 {
            return Err(AddressSpaceError::HugePageUnsupported);
        }
        let pd = unsafe { table_pointer(pdpt_entry)? };
        // SAFETY: `pd` was validated as an identity-reachable table.
        let pd_entry = unsafe { pd.add(indices[2]).read() };
        if pd_entry & user_present != user_present {
            return Err(AddressSpaceError::PermissionMismatch);
        }
        if pd_entry & HUGE != 0 {
            return Err(AddressSpaceError::HugePageUnsupported);
        }
        let pt = unsafe { table_pointer(pd_entry)? };
        // SAFETY: `pt` was validated as an identity-reachable leaf table.
        let pte = unsafe { pt.add(indices[3]).read() };
        let expected = permissions.flags();
        if pte & user_present != user_present
            || pte & ADDRESS_MASK != (physical_frame & ADDRESS_MASK)
            || pte & (WRITABLE | NX) != (expected & (WRITABLE | NX))
        {
            return Err(AddressSpaceError::PermissionMismatch);
        }
        Ok(())
    }
}

fn validate_user_mapping(
    virtual_address: u64,
    physical_frame: u64,
) -> Result<[usize; 4], AddressSpaceError> {
    if virtual_address % PAGE_SIZE != 0 || physical_frame % PAGE_SIZE != 0 {
        return Err(AddressSpaceError::UnalignedAddress);
    }
    if !is_user_address(virtual_address) {
        return Err(AddressSpaceError::InvalidUserAddress);
    }
    if physical_frame >= IDENTITY_MAP_BYTES {
        return Err(AddressSpaceError::FrameOutsideIdentityWindow);
    }
    Ok(page_table_indices(virtual_address))
}

unsafe fn table_pointer(entry: u64) -> Result<*mut u64, AddressSpaceError> {
    let address = entry & ADDRESS_MASK;
    if address >= IDENTITY_MAP_BYTES {
        return Err(AddressSpaceError::FrameOutsideIdentityWindow);
    }
    Ok(address as *mut u64)
}

/// Read CR3 from ring 0.
///
/// # Safety
///
/// Only valid in ring 0.
pub unsafe fn read_cr3() -> u64 {
    let value: u64;
    // SAFETY: Reading CR3 is permitted in ring 0.
    unsafe { core::arch::asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags)) };
    value
}

/// Load a PML4 root into CR3, flushing non-global TLB entries.
///
/// # Safety
///
/// `root` must be a valid PML4 whose tables are identity-accessible.
pub unsafe fn write_cr3(root: u64) {
    // SAFETY: The caller guarantees a valid PML4 root.
    unsafe { core::arch::asm!("mov cr3, {}", in(reg) root, options(nostack, preserves_flags)) };
}

/// TLB-shootdown generation. Generation zero means "no shootdown issued".
static SHOOTDOWN_GENERATION: AtomicU64 = AtomicU64::new(0);
/// Last generation acknowledged by each CPU slot.
static SHOOTDOWN_ACKS: [AtomicU64; crate::interrupts::MAX_CPUS] =
    [const { AtomicU64::new(0) }; crate::interrupts::MAX_CPUS];
static SHOOTDOWN_ACK_COUNT: AtomicU64 = AtomicU64::new(0);

/// Statistics returned by a shootdown observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShootdownStats {
    pub generation: u64,
    pub targets: u32,
    pub acks: u32,
}

/// Acknowledgement hook for the dedicated TLB-shootdown IPI vector.
///
/// The interrupt handler calls this after reloading CR3 locally. Publishing the
/// current generation with Release is the ACK the initiator acquires.
pub fn ack_tlb_shootdown() {
    let cpu_index = crate::scheduler::current_cpu_index();
    if cpu_index >= crate::interrupts::MAX_CPUS {
        return;
    }
    let generation = SHOOTDOWN_GENERATION.load(Ordering::Acquire);
    if generation == 0 {
        return;
    }
    SHOOTDOWN_ACKS[cpu_index].store(generation, Ordering::Release);
    SHOOTDOWN_ACK_COUNT.fetch_add(1, Ordering::AcqRel);
}

pub fn tlb_shootdown_ack_count() -> u64 {
    SHOOTDOWN_ACK_COUNT.load(Ordering::Acquire)
}

/// Count the remote CPUs that must acknowledge a generation. The initiator
/// reloads CR3 locally and never sends itself an IPI, so it is not a target:
/// the reaper may run with local interrupts masked and must never spin on self.
fn remote_shootdown_targets(target_mask: u64) -> u32 {
    let initiator = crate::scheduler::current_cpu_index();
    let mut targets = 0u32;
    for cpu_index in 0..crate::interrupts::MAX_CPUS {
        if cpu_index != initiator && target_mask & (1u64 << cpu_index) != 0 {
            targets += 1;
        }
    }
    targets
}

/// Non-blocking ACK observation for a previously issued generation.
pub fn tlb_shootdown_status(target_mask: u64, generation: u64) -> ShootdownStats {
    let initiator = crate::scheduler::current_cpu_index();
    let mut targets = 0u32;
    let mut acks = 0u32;
    for cpu_index in 0..crate::interrupts::MAX_CPUS {
        if cpu_index == initiator || target_mask & (1u64 << cpu_index) == 0 {
            continue;
        }
        targets += 1;
        if generation != 0 && SHOOTDOWN_ACKS[cpu_index].load(Ordering::Acquire) >= generation {
            acks += 1;
        }
    }
    ShootdownStats { generation, targets, acks }
}

/// Invalidate stale translations of `root` on every remote CPU in `target_mask`.
///
/// This never spin-waits: the caller requeues the process when ACKs are still
/// outstanding. Each targeted CPU reloads CR3 in its interrupt handler before
/// publishing its ACK, so frames are released only after every target has
/// stopped referencing them. The initiator invalidates locally before sending.
pub fn tlb_shootdown(_root: u64, target_mask: u64) -> ShootdownStats {
    let generation = SHOOTDOWN_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    let initiator = crate::scheduler::current_cpu_index();
    for cpu_index in 0..crate::interrupts::MAX_CPUS {
        if cpu_index == initiator || target_mask & (1u64 << cpu_index) == 0 {
            continue;
        }
        if let Some(apic_id) = crate::apic::apic_id_for_cpu_index(cpu_index) {
            let _ = crate::apic::send_ipi(apic_id, crate::apic::TLB_SHOOTDOWN_VECTOR);
        }
    }

    // The initiator drops its own stale translations before requeueing.
    // SAFETY: CR3 remains a valid kernel-owned PML4 throughout.
    unsafe {
        let current = read_cr3();
        write_cr3(current);
    }

    let mut status = tlb_shootdown_status(target_mask, generation);
    status.acks = status.acks.min(remote_shootdown_targets(target_mask));
    status
}

/// Check that every page in a user range is present and user-accessible.
///
/// # Safety
///
/// The active CR3 and all page-table frames must be valid and identity-mapped.
pub unsafe fn user_range_is_mapped(address: u64, size: u64, writable: bool) -> bool {
    if !crate::uaccess::is_user_range(address, size) {
        return false;
    }

    let end = match address.checked_add(size) {
        Some(end) => end,
        None => return false,
    };
    let root = cr3_root(unsafe { read_cr3() });
    if root >= IDENTITY_MAP_BYTES || root % PAGE_SIZE != 0 {
        return false;
    }

    let first_page = address & !(PAGE_SIZE - 1);
    let last_page = (end - 1) & !(PAGE_SIZE - 1);
    let mut page = first_page;
    loop {
        let indices = page_table_indices(page);
        let mut table = root as *const u64;
        for (level, index) in indices.iter().enumerate() {
            let entry = unsafe { table.add(*index).read_volatile() };
            if entry & (PRESENT | USER) != (PRESENT | USER)
                || (writable && entry & WRITABLE == 0)
                || (level < 3 && entry & HUGE != 0)
            {
                return false;
            }
            if level < 3 {
                table = match unsafe { table_pointer(entry) } {
                    Ok(next) => next as *const u64,
                    Err(_) => return false,
                };
            }
        }

        if page == last_page {
            return true;
        }
        page = match page.checked_add(PAGE_SIZE) {
            Some(next) => next,
            None => return false,
        };
    }
}

/// Boot-time proof of user execution, user-stack access, and a controlled
/// DPL3 interrupt return to the kernel through the TSS privilege stack.
///
/// # Safety
///
/// Must run after the kernel page tables and TSS/IDT are active. The caller
/// must ensure no other CPU can use the global test transition state.
#[cfg(feature = "qemu-ring3-test")]
pub unsafe fn run_qemu_ring3_test(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
) -> Result<(), AddressSpaceError> {
    let free_pages_before = allocator.free_pages();
    let code_frame = allocator
        .allocate_frame()
        .ok_or(AddressSpaceError::OutOfFrames)?;
    let stack_frame = allocator
        .allocate_frame()
        .ok_or(AddressSpaceError::OutOfFrames)?;
    if code_frame % PAGE_SIZE != 0 || stack_frame % PAGE_SIZE != 0 {
        return Err(AddressSpaceError::InvalidFrameAlignment);
    }
    if code_frame >= IDENTITY_MAP_BYTES || stack_frame >= IDENTITY_MAP_BYTES {
        return Err(AddressSpaceError::FrameOutsideIdentityWindow);
    }

    let mut code = [0u8; 17];
    code[0] = 0x48;
    code[1] = 0xB8;
    code[2..10].copy_from_slice(&RING3_TEST_MAGIC.to_le_bytes());
    code[10..15].copy_from_slice(&[0x48, 0x89, 0x44, 0x24, 0xF8]);
    code[15..17].copy_from_slice(&[0xCD, 0x80]);
    // SAFETY: Both are freshly allocated identity-accessible frames.
    unsafe {
        core::ptr::write_bytes(code_frame as *mut u8, 0, PAGE_SIZE as usize);
        core::ptr::write_bytes(stack_frame as *mut u8, 0, PAGE_SIZE as usize);
        core::ptr::copy_nonoverlapping(code.as_ptr(), code_frame as *mut u8, code.len());
    }

    // SAFETY: The kernel root is active and the allocator owns free frames.
    let mut space = unsafe { AddressSpace::create(allocator, kernel_root)? };
    // SAFETY: The virtual addresses and backing frames were validated above.
    unsafe {
        space.map_user_page(
            allocator,
            RING3_CODE_ADDRESS,
            code_frame,
            UserPagePermissions::ReadOnlyExecute,
        )?;
        space.map_user_page(
            allocator,
            RING3_STACK_ADDRESS,
            stack_frame,
            UserPagePermissions::ReadWriteNoExecute,
        )?;
        space.verify_user_mapping(
            RING3_CODE_ADDRESS,
            code_frame,
            UserPagePermissions::ReadOnlyExecute,
        )?;
        space.verify_user_mapping(
            RING3_STACK_ADDRESS,
            stack_frame,
            UserPagePermissions::ReadWriteNoExecute,
        )?;
    }

    let user_rsp = RING3_STACK_ADDRESS + PAGE_SIZE - 16;
    let previous_root = unsafe { read_cr3() };
    let previous_flags: u64;
    // SAFETY: This test is single-core and must not be preempted while swapping CR3.
    unsafe {
        core::arch::asm!("pushfq; pop {}", out(reg) previous_flags, options(nomem, nostack, preserves_flags));
        core::arch::asm!("cli", options(nomem, nostack));
        write_cr3(space.root());
    }

    let transition_result = if cr3_root(unsafe { read_cr3() }) != space.root() {
        Err(AddressSpaceError::Cr3Mismatch)
    } else if !unsafe {
        crate::interrupts::enter_ring3_test_context(RING3_CODE_ADDRESS, user_rsp)
    } {
        Err(AddressSpaceError::Ring3TransitionFailed)
    } else {
        let stack_value = {
            let _smap_guard = crate::uaccess::UserAccessGuard::new();
            // SAFETY: The ring-3 program wrote this word in its mapped user stack.
            unsafe { core::ptr::read_volatile((user_rsp - 8) as *const u64) }
        };
        if stack_value == RING3_TEST_MAGIC {
            Ok(())
        } else {
            Err(AddressSpaceError::Ring3StackWriteMismatch)
        }
    };

    // SAFETY: Restore the original address space before reclaiming test frames.
    unsafe { write_cr3(previous_root) };
    // SAFETY: The kernel root is active, so none of the test process frames are in use.
    let reclaim_result = unsafe { space.destroy(allocator) };
    let frames_reclaimed = allocator.free_pages() == free_pages_before;
    if previous_flags & (1 << 9) != 0 {
        // SAFETY: The previous root is restored and the caller had interrupts enabled.
        unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
    }
    reclaim_result?;
    if !frames_reclaimed {
        return Err(AddressSpaceError::FrameReclaimMismatch);
    }
    transition_result
}

/// Boot-time proof: SYSCALL/SYSRET fast path from Ring 3.
///
/// # Safety
///
/// Must run after the kernel page tables, TSS/IDT, and SYSCALL MSRs are active.
/// The caller must ensure no other CPU can use the global test transition state.
#[cfg(feature = "qemu-syscall-test")]
pub unsafe fn run_qemu_syscall_test(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
) -> Result<(), AddressSpaceError> {
    fn emit(code: &mut [u8], length: &mut usize, bytes: &[u8]) {
        let end = *length + bytes.len();
        code[*length..end].copy_from_slice(bytes);
        *length = end;
    }

    fn emit_number(code: &mut [u8], length: &mut usize, number: u32) {
        emit(code, length, &[0xB8]);
        emit(code, length, &number.to_le_bytes());
    }

    fn emit_syscall(code: &mut [u8], length: &mut usize, number: u32) {
        emit_number(code, length, number);
        emit(code, length, &[0x0F, 0x05]);
    }

    fn emit_result_store(code: &mut [u8], length: &mut usize, offset: i8) {
        emit(code, length, &[0x48, 0x89, 0x44, 0x24, offset as u8]);
    }

    let free_pages_before = allocator.free_pages();
    let code_frame = allocator
        .allocate_frame()
        .ok_or(AddressSpaceError::OutOfFrames)?;
    let stack_frame = allocator
        .allocate_frame()
        .ok_or(AddressSpaceError::OutOfFrames)?;
    if code_frame % PAGE_SIZE != 0 || stack_frame % PAGE_SIZE != 0 {
        return Err(AddressSpaceError::InvalidFrameAlignment);
    }
    if code_frame >= IDENTITY_MAP_BYTES || stack_frame >= IDENTITY_MAP_BYTES {
        return Err(AddressSpaceError::FrameOutsideIdentityWindow);
    }

    let mut code = [0u8; 128];
    let mut code_len = 0;
    emit_syscall(
        &mut code,
        &mut code_len,
        crate::syscall::SYS_GET_API_VERSION as u32,
    );
    emit_result_store(&mut code, &mut code_len, -8);

    for (number, slot) in [
        (crate::syscall::SYS_WRITE as u32, -16),
        (crate::syscall::SYS_READ as u32, -24),
    ] {
        emit_number(&mut code, &mut code_len, number);
        emit(&mut code, &mut code_len, &[0xBF, 1, 0, 0, 0]);
        emit(&mut code, &mut code_len, &[0x48, 0xBE]);
        emit(
            &mut code,
            &mut code_len,
            &crate::address_space::USER_LIMIT.to_le_bytes(),
        );
        emit(&mut code, &mut code_len, &[0xBA, 1, 0, 0, 0, 0x0F, 0x05]);
        emit_result_store(&mut code, &mut code_len, slot);
    }

    emit_syscall(&mut code, &mut code_len, u32::MAX);
    emit_result_store(&mut code, &mut code_len, -32);
    emit_syscall(&mut code, &mut code_len, crate::syscall::SYS_YIELD as u32);
    emit_result_store(&mut code, &mut code_len, -40);
    emit_number(&mut code, &mut code_len, crate::syscall::SYS_EXIT as u32);
    emit(&mut code, &mut code_len, &[0xBF, 42, 0, 0, 0]);
    emit(&mut code, &mut code_len, &[0x0F, 0x05]);

    // SAFETY: Both are freshly allocated identity-accessible frames.
    unsafe {
        core::ptr::write_bytes(code_frame as *mut u8, 0, PAGE_SIZE as usize);
        core::ptr::write_bytes(stack_frame as *mut u8, 0, PAGE_SIZE as usize);
        core::ptr::copy_nonoverlapping(code.as_ptr(), code_frame as *mut u8, code_len);
    }

    // SAFETY: The kernel root is active and the allocator owns free frames.
    let mut space = unsafe { AddressSpace::create(allocator, kernel_root)? };
    // SAFETY: The mapping addresses and backing frames were validated above.
    unsafe {
        space.map_user_page(
            allocator,
            SYSCALL_CODE_ADDRESS,
            code_frame,
            UserPagePermissions::ReadOnlyExecute,
        )?;
        space.map_user_page(
            allocator,
            SYSCALL_STACK_ADDRESS,
            stack_frame,
            UserPagePermissions::ReadWriteNoExecute,
        )?;
        space.verify_user_mapping(
            SYSCALL_CODE_ADDRESS,
            code_frame,
            UserPagePermissions::ReadOnlyExecute,
        )?;
        space.verify_user_mapping(
            SYSCALL_STACK_ADDRESS,
            stack_frame,
            UserPagePermissions::ReadWriteNoExecute,
        )?;
    }

    let user_rsp = SYSCALL_STACK_ADDRESS + PAGE_SIZE - 16;
    let previous_root = unsafe { read_cr3() };
    let previous_flags: u64;
    // SAFETY: This test is single-core and must not be preempted while swapping CR3.
    unsafe {
        core::arch::asm!("pushfq; pop {}", out(reg) previous_flags, options(nomem, nostack, preserves_flags));
        core::arch::asm!("cli", options(nomem, nostack));
        write_cr3(space.root());
    }

    let transition_result = if cr3_root(unsafe { read_cr3() }) != space.root() {
        Err(AddressSpaceError::Cr3Mismatch)
    } else {
        // SAFETY: The syscall test code and stack are mapped in the active address space.
        let exit_status =
            unsafe { crate::interrupts::enter_user_mode(SYSCALL_CODE_ADDRESS, user_rsp) };
        let read_result = |offset: i64| {
            let _smap_guard = crate::uaccess::UserAccessGuard::new();
            // SAFETY: The Ring 3 test stores each result in the mapped user stack.
            unsafe { core::ptr::read_volatile((user_rsp as i64 + offset) as *const u64) }
        };
        let results = [
            read_result(-8),
            read_result(-16),
            read_result(-24),
            read_result(-32),
            read_result(-40),
        ];
        if results[0] != crate::syscall::API_VERSION {
            Err(AddressSpaceError::SyscallReturnMismatch)
        } else if results[1..4]
            != [
                crate::syscall::ERROR_BAD_ADDRESS,
                crate::syscall::ERROR_BAD_ADDRESS,
                crate::syscall::ERROR_NO_SYS,
            ]
        {
            Err(AddressSpaceError::SyscallNegativeTestFailed)
        } else if results[4] != 0 {
            Err(AddressSpaceError::SyscallReturnMismatch)
        } else if exit_status != 42 {
            Err(AddressSpaceError::UserProcessExitFailed)
        } else {
            Ok(())
        }
    };

    // SAFETY: Restore the original address space before reclaiming process frames.
    unsafe { write_cr3(previous_root) };
    // SAFETY: The kernel root is active, so none of the process-owned frames are in use.
    let reclaim_result = unsafe { space.destroy(allocator) };
    let frames_reclaimed = allocator.free_pages() == free_pages_before;
    if previous_flags & (1 << 9) != 0 {
        // SAFETY: The kernel root is restored and the caller had interrupts enabled.
        unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
    }
    reclaim_result?;
    if !frames_reclaimed {
        return Err(AddressSpaceError::FrameReclaimMismatch);
    }
    transition_result
}

#[cfg(feature = "qemu-ring3-smp-test")]
const RING3_SMP_MAX_CPUS: usize = 64;
#[cfg(feature = "qemu-ring3-smp-test")]
const RING3_SMP_CODE_ADDRESS: u64 = USER_BASE + 0xA000;
#[cfg(feature = "qemu-ring3-smp-test")]
const RING3_SMP_BARRIER_ADDRESS: u64 = USER_BASE + 0xB000;
#[cfg(feature = "qemu-ring3-smp-test")]
const RING3_SMP_FAULT_CODE_ADDRESS: u64 = USER_BASE + 0xC000;
#[cfg(feature = "qemu-ring3-smp-test")]
const RING3_SMP_FAULT_EXIT_STATUS: u64 = 0xF001_FA01_7FA0_1701;
#[cfg(feature = "qemu-ring3-smp-test")]
const RING3_SMP_STACK_BASE: u64 = USER_BASE + 0x1_0000;
#[cfg(feature = "qemu-ring3-smp-test")]
const RING3_SMP_EXIT_STATUS: u64 = 42;

#[cfg(feature = "qemu-ring3-smp-test")]
static RING3_SMP_TEST_ROOT: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "qemu-ring3-smp-test")]
static RING3_SMP_TEST_START: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "qemu-ring3-smp-test")]
static RING3_SMP_TEST_DONE: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "qemu-ring3-smp-test")]
static RING3_SMP_TEST_IRQS: [AtomicU64; RING3_SMP_MAX_CPUS] =
    [const { AtomicU64::new(0) }; RING3_SMP_MAX_CPUS];
#[cfg(feature = "qemu-ring3-smp-test")]
static RING3_SMP_TEST_RESULTS: [AtomicU64; RING3_SMP_MAX_CPUS] =
    [const { AtomicU64::new(0) }; RING3_SMP_MAX_CPUS];

#[cfg(feature = "qemu-ring3-smp-test")]
pub struct Ring3SmpTestPlan {
    space: AddressSpace,
    free_pages_before: u64,
    barrier_frame: u64,
}

/// Allocate one shared user address space with a private user stack for every
/// supported APIC-ID slot. All CPUs execute the same syscall sequence while a
/// shared user-mode barrier guarantees their SYSCALL instructions overlap.
#[cfg(feature = "qemu-ring3-smp-test")]
pub unsafe fn prepare_qemu_ring3_smp_test(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
) -> Result<Ring3SmpTestPlan, AddressSpaceError> {
    let free_pages_before = allocator.free_pages();
    let code_frame = allocator.allocate_frame().ok_or(AddressSpaceError::OutOfFrames)?;
    let barrier_frame = allocator.allocate_frame().ok_or(AddressSpaceError::OutOfFrames)?;
    let fault_code_frame = allocator.allocate_frame().ok_or(AddressSpaceError::OutOfFrames)?;
    let mut stack_frames = Vec::new();
    stack_frames.try_reserve(RING3_SMP_MAX_CPUS).map_err(|_| AddressSpaceError::OutOfMemory)?;
    for _ in 0..RING3_SMP_MAX_CPUS {
        stack_frames.push(allocator.allocate_frame().ok_or(AddressSpaceError::OutOfFrames)?);
    }
    for frame in core::iter::once(&code_frame)
        .chain(core::iter::once(&barrier_frame))
        .chain(core::iter::once(&fault_code_frame))
        .chain(stack_frames.iter())
    {
        if *frame % PAGE_SIZE != 0 {
            return Err(AddressSpaceError::InvalidFrameAlignment);
        }
        if *frame >= IDENTITY_MAP_BYTES {
            return Err(AddressSpaceError::FrameOutsideIdentityWindow);
        }
        // SAFETY: Every frame was just allocated and is identity accessible.
        unsafe { core::ptr::write_bytes(*frame as *mut u8, 0, PAGE_SIZE as usize) };
    }
    for (cpu_index, frame) in stack_frames.iter().enumerate() {
        // Give every CPU a distinct SYS_EXIT status so a shared exit-status
        // variable cannot accidentally pass the SMP test. The status lives at
        // user_rsp - 16, the slot the user code reads for the SYS_EXIT argument.
        let status = (RING3_SMP_EXIT_STATUS + cpu_index as u64) as u32;
        // SAFETY: The stack frame is freshly allocated and identity accessible.
        unsafe {
            core::ptr::write_volatile(
                (*frame as *mut u8).add(PAGE_SIZE as usize - 32).cast::<u32>(),
                status,
            );
        }
    }

    // User code: increment ready_count; wait until it equals expected_count;
    // call SYS_GET_API_VERSION; BSP/even APIC IDs use SYS_EXIT, odd APIC IDs use INT 0x80.
    let mut code = [0u8; 160];
    let mut length = 0usize;
    let barrier = RING3_SMP_BARRIER_ADDRESS.to_le_bytes();
    code[length..length + 2].copy_from_slice(&[0x48, 0xBB]); // mov rbx, imm64
    length += 2;
    code[length..length + 8].copy_from_slice(&barrier);
    length += 8;
    code[length..length + 4].copy_from_slice(&[0xF0, 0x48, 0xFF, 0x03]); // lock inc [rbx]
    length += 4;
    code[length..length + 3].copy_from_slice(&[0x48, 0x8B, 0x03]); // mov rax, [rbx]
    length += 3;
    code[length..length + 4].copy_from_slice(&[0x48, 0x3B, 0x43, 0x08]); // cmp rax, [rbx+8]
    length += 4;
    code[length..length + 2].copy_from_slice(&[0x75, 0xF7]); // jne back to ready check
    length += 2;

    #[cfg(any(feature = "qemu-ring3-fault-pf-test", feature = "qemu-ring3-fault-gp-test"))]
    {
        let bsp_rsp = RING3_SMP_STACK_BASE
            + crate::interrupts::bootstrap_cpu_index() as u64 * PAGE_SIZE
            + PAGE_SIZE - 16;
        code[length..length + 3].copy_from_slice(&[0x48, 0x89, 0xE0]); // mov rax, rsp
        length += 3;
        code[length..length + 2].copy_from_slice(&[0x48, 0xBB]); // movabs rbx, BSP RSP
        length += 2;
        code[length..length + 8].copy_from_slice(&bsp_rsp.to_le_bytes());
        length += 8;
        code[length..length + 3].copy_from_slice(&[0x48, 0x39, 0xD8]); // cmp rax, rbx
        length += 3;
        code[length..length + 2].copy_from_slice(&[0x75, 0x0C]); // other CPUs skip the 12-byte call
        length += 2;
        code[length..length + 2].copy_from_slice(&[0x48, 0xB8]); // movabs rax, fault stub
        length += 2;
        code[length..length + 8].copy_from_slice(&RING3_SMP_FAULT_CODE_ADDRESS.to_le_bytes());
        length += 8;
        code[length..length + 2].copy_from_slice(&[0xFF, 0xD0]); // call rax
        length += 2;
        // Non-BSP participants execute a verified syscall burst while the BSP faults.
        code[length..length + 6].copy_from_slice(&[0x41, 0xBC, 0x10, 0x27, 0x00, 0x00]); // mov r12d, 10,000
        length += 6;
        let syscall_burst = [
            0xB8, 0x00, 0x00, 0x00, 0x00, // mov eax, SYS_GET_API_VERSION
            0x0F, 0x05,                   // syscall
            0x48, 0x83, 0xF8, 0x01,       // cmp rax, API_VERSION
            0x74, 0x02,                   // je over ud2
            0x0F, 0x0B,                   // ud2 on a corrupted syscall result
            0x49, 0xFF, 0xCC,             // dec r12
            0x75, 0xEC,                   // jnz syscall_burst
        ];
        code[length..length + syscall_burst.len()].copy_from_slice(&syscall_burst);
        length += syscall_burst.len();
    }
    code[length..length + 5].copy_from_slice(&[0xB9, 0x80, 0x96, 0x98, 0x00]); // mov ecx, 10,000,000
    length += 5;
    code[length..length + 2].copy_from_slice(&[0xF3, 0x90]); // pause while user IF is enabled
    length += 2;
    code[length..length + 2].copy_from_slice(&[0xE2, 0xFC]); // loop back to pause
    length += 2;
    code[length] = 0xB8; // mov eax, SYS_GET_API_VERSION
    length += 1;
    code[length..length + 4].copy_from_slice(&(crate::syscall::SYS_GET_API_VERSION as u32).to_le_bytes());
    length += 4;
    code[length..length + 2].copy_from_slice(&[0x0F, 0x05]); // syscall
    length += 2;
    code[length..length + 5].copy_from_slice(&[0x48, 0x89, 0x44, 0x24, 0xF8]); // mov [rsp-8], rax
    length += 5;
    code[length..length + 4].copy_from_slice(&[0x8B, 0x44, 0x24, 0xE8]); // mov eax, [rsp-24]
    length += 4;
    code[length..length + 2].copy_from_slice(&[0x85, 0xC0]); // test eax, eax
    length += 2;
    code[length..length + 2].copy_from_slice(&[0x75, 0x0C]); // SYS_EXIT participants skip the int 0x80 exit
    length += 2;
    code[length..length + 2].copy_from_slice(&[0x48, 0xB8]); // movabs rax, Ring 3 exit magic
    length += 2;
    code[length..length + 8].copy_from_slice(&RING3_TEST_MAGIC.to_le_bytes());
    length += 8;
    code[length..length + 2].copy_from_slice(&[0xCD, 0x80]); // int 0x80 exit path
    length += 2;
    code[length] = 0xB8; // mov eax, SYS_EXIT
    length += 1;
    code[length..length + 4].copy_from_slice(&(crate::syscall::SYS_EXIT as u32).to_le_bytes());
    length += 4;
    code[length..length + 4].copy_from_slice(&[0x8B, 0x7C, 0x24, 0xF0]); // mov edi, [rsp-16]: status slot
    length += 4;
    code[length..length + 2].copy_from_slice(&[0x0F, 0x05]); // syscall
    length += 2;
    code[length..length + 2].copy_from_slice(&[0x0F, 0x0B]); // ud2 if SYS_EXIT returns
    length += 2;

    #[cfg(feature = "qemu-ring3-fault-pf-test")]
    unsafe {
        core::ptr::write_bytes(fault_code_frame as *mut u8, 0x90, PAGE_SIZE as usize);
        let mut stub = [0x48, 0xB8, 0, 0, 0, 0, 0, 0, 0, 0, 0x48, 0x8B, 0x00];
        stub[2..10].copy_from_slice(&(USER_BASE + 0x5000).to_le_bytes());
        core::ptr::copy_nonoverlapping(stub.as_ptr(), fault_code_frame as *mut u8, stub.len());
    }
    #[cfg(feature = "qemu-ring3-fault-gp-test")]
    unsafe {
        core::ptr::write_bytes(fault_code_frame as *mut u8, 0x90, PAGE_SIZE as usize);
        core::ptr::write_volatile(fault_code_frame as *mut u8, 0xF4); // HLT at CPL3 -> #GP
    }
    // SAFETY: The code frame is freshly allocated and identity accessible.
    unsafe { core::ptr::copy_nonoverlapping(code.as_ptr(), code_frame as *mut u8, length) };

    // SAFETY: The kernel root is active and the allocator owns every frame.
    let mut space = unsafe { AddressSpace::create(allocator, kernel_root)? };
    // SAFETY: The mapping addresses and backing frames are page-aligned and validated.
    unsafe {
        space.map_user_page(allocator, RING3_SMP_CODE_ADDRESS, code_frame, UserPagePermissions::ReadOnlyExecute)?;
        space.map_user_page(allocator, RING3_SMP_BARRIER_ADDRESS, barrier_frame, UserPagePermissions::ReadWriteNoExecute)?;
        space.map_user_page(allocator, RING3_SMP_FAULT_CODE_ADDRESS, fault_code_frame, UserPagePermissions::ReadOnlyExecute)?;
        for (cpu_index, frame) in stack_frames.iter().enumerate() {
            space.map_user_page(
                allocator,
                RING3_SMP_STACK_BASE + cpu_index as u64 * PAGE_SIZE,
                *frame,
                UserPagePermissions::ReadWriteNoExecute,
            )?;
        }
        space.verify_user_mapping(RING3_SMP_CODE_ADDRESS, code_frame, UserPagePermissions::ReadOnlyExecute)?;
        space.verify_user_mapping(RING3_SMP_BARRIER_ADDRESS, barrier_frame, UserPagePermissions::ReadWriteNoExecute)?;
    }

    for result in &RING3_SMP_TEST_RESULTS {
        result.store(0, Ordering::Release);
    }
    for irq_count in &RING3_SMP_TEST_IRQS {
        irq_count.store(0, Ordering::Release);
    }
    RING3_SMP_TEST_DONE.store(0, Ordering::Release);
    RING3_SMP_TEST_START.store(false, Ordering::Release);
    RING3_SMP_TEST_ROOT.store(space.root(), Ordering::Release);

    Ok(Ring3SmpTestPlan { space, free_pages_before, barrier_frame })
}

/// Record an APIC timer interrupt that interrupted Ring 3 in the SMP test.
#[cfg(feature = "qemu-ring3-smp-test")]
pub(crate) fn record_qemu_ring3_smp_user_irq(cpu_index: usize) {
    if cpu_index < RING3_SMP_MAX_CPUS {
        RING3_SMP_TEST_IRQS[cpu_index].fetch_add(1, Ordering::AcqRel);
    }
}

/// AP-side rendezvous. APs publish their parked state before waiting here so
/// the BSP can finish SMP bring-up and release every participant together.
#[cfg(feature = "qemu-ring3-smp-test")]
pub(crate) fn wait_and_run_qemu_ring3_smp_test(cpu_index: usize) {
    while !RING3_SMP_TEST_START.load(Ordering::Acquire) {
        core::hint::spin_loop();
    }
    run_qemu_ring3_smp_test_cpu(cpu_index);
}

#[cfg(feature = "qemu-ring3-smp-test")]
fn run_qemu_ring3_smp_test_cpu(cpu_index: usize) {
    if cpu_index >= RING3_SMP_MAX_CPUS {
        return;
    }
    let root = RING3_SMP_TEST_ROOT.load(Ordering::Acquire);
    let user_rsp = RING3_SMP_STACK_BASE + cpu_index as u64 * PAGE_SIZE + PAGE_SIZE - 16;
    let previous_root = unsafe { read_cr3() };
    let previous_flags: u64;
    // SAFETY: Each participant switches only its own CR3 and enters Ring 3 with IRQs masked.
    unsafe {
        core::arch::asm!("pushfq; pop {}; cli", out(reg) previous_flags);
        write_cr3(root);
    }
    let is_bsp = crate::apic::is_bootstrap_cpu();
    // Fault-recovery matrix: keep non-faulting peers on the INT 0x80 exit
    // path after their syscall burst. SYS_EXIT remains covered by the regular
    // Ring 3 SMP matrix; mixing its independent exit-status assertion into the
    // fault rendezvous obscures whether peer syscalls survived the BSP fault.
    let use_sys_exit = {
        #[cfg(any(feature = "qemu-ring3-fault-pf-test", feature = "qemu-ring3-fault-gp-test"))]
        { is_bsp }
        #[cfg(not(any(feature = "qemu-ring3-fault-pf-test", feature = "qemu-ring3-fault-gp-test")))]
        { is_bsp || cpu_index & 1 == 0 }
    };
    let user_irq_before = RING3_SMP_TEST_IRQS[cpu_index].load(Ordering::Acquire);
    {
        let _smap_guard = crate::uaccess::UserAccessGuard::new();
        // SAFETY: This CPU owns its user stack page in the shared test address space.
        unsafe {
            core::ptr::write_volatile((user_rsp - 24) as *mut u32, if use_sys_exit { 1 } else { 0 });
        }
    }
    let exit_ok = if use_sys_exit {
        let exit_status = unsafe { crate::interrupts::enter_user_mode(RING3_SMP_CODE_ADDRESS, user_rsp) };
        #[cfg(any(feature = "qemu-ring3-fault-pf-test", feature = "qemu-ring3-fault-gp-test"))]
        if cpu_index == crate::interrupts::bootstrap_cpu_index() {
            let mut repeated_ok = exit_status == RING3_SMP_FAULT_EXIT_STATUS;
            for _ in 1..10 {
                let repeated = unsafe { crate::interrupts::enter_user_mode(RING3_SMP_FAULT_CODE_ADDRESS, user_rsp) };
                repeated_ok &= repeated == RING3_SMP_FAULT_EXIT_STATUS;
            }
            let fault_info_ok = crate::interrupts::last_user_fault().is_some_and(|fault| {
                (cfg!(feature = "qemu-ring3-fault-pf-test")
                    && fault.vector == 14
                    && fault.error_code == 0x4
                    && fault.cr2 == USER_BASE + 0x5000
                    && fault.rip == RING3_SMP_FAULT_CODE_ADDRESS + 10)
                    || (cfg!(feature = "qemu-ring3-fault-gp-test")
                        && fault.vector == 13
                        && fault.error_code == 0
                        && fault.cr2 == 0
                        && fault.rip == RING3_SMP_FAULT_CODE_ADDRESS)
            });
            repeated_ok &= fault_info_ok;
            if repeated_ok {
                unsafe { crate::console::debug_write(b"SYSTEMES_TEST: Ring 3 fault returned to kernel; repeated=10\r\n"); }
            }
            repeated_ok
        } else { exit_status == RING3_SMP_EXIT_STATUS + cpu_index as u64 }
        #[cfg(not(any(feature = "qemu-ring3-fault-pf-test", feature = "qemu-ring3-fault-gp-test")))]
        { exit_status == RING3_SMP_EXIT_STATUS + cpu_index as u64 }
    } else {
        // Odd APs exercise the DPL3 INT 0x80 exit path concurrently.
        unsafe { crate::interrupts::enter_ring3_test_context(RING3_SMP_CODE_ADDRESS, user_rsp) }
    };
    let observed_api = {
        #[cfg(any(feature = "qemu-ring3-fault-pf-test", feature = "qemu-ring3-fault-gp-test"))]
        if cpu_index == crate::interrupts::bootstrap_cpu_index() { crate::syscall::API_VERSION } else {
            let _smap_guard = crate::uaccess::UserAccessGuard::new();
            unsafe { core::ptr::read_volatile((user_rsp - 8) as *const u64) }
        }
        #[cfg(not(any(feature = "qemu-ring3-fault-pf-test", feature = "qemu-ring3-fault-gp-test")))]
        {
            let _smap_guard = crate::uaccess::UserAccessGuard::new();
            unsafe { core::ptr::read_volatile((user_rsp - 8) as *const u64) }
        }
    };
    // SAFETY: Restore this CPU's previous root before publishing completion.
    unsafe { write_cr3(previous_root) };
    if previous_flags & (1 << 9) != 0 {
        // SAFETY: Restore the caller's original interrupt state after leaving the shared test root.
        unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
    }
    let gs_restored = crate::interrupts::verify_syscall_configuration();
    let user_irq_after = RING3_SMP_TEST_IRQS[cpu_index].load(Ordering::Acquire);
    let user_irq_ok = !use_sys_exit
        || {
            #[cfg(any(feature = "qemu-ring3-fault-pf-test", feature = "qemu-ring3-fault-gp-test"))]
            if cpu_index == crate::interrupts::bootstrap_cpu_index() { true } else { user_irq_after > user_irq_before }
            #[cfg(not(any(feature = "qemu-ring3-fault-pf-test", feature = "qemu-ring3-fault-gp-test")))]
            { user_irq_after > user_irq_before }
        };
    let failure_bits = (if exit_ok { 0 } else { 1 })
        | (if observed_api == crate::syscall::API_VERSION { 0 } else { 2 })
        | (if gs_restored { 0 } else { 4 })
        | (if user_irq_ok { 0 } else { 8 });
    let success = failure_bits == 0;
    RING3_SMP_TEST_RESULTS[cpu_index]
        .store(if success { 1 } else { 0x100 | failure_bits }, Ordering::Release);
    RING3_SMP_TEST_DONE.fetch_add(1, Ordering::AcqRel);
}

/// QEMU diagnostic bitmask: bit 0 exit path, bit 1 API result, bit 2 GS restore,
/// bit 3 Ring 3-origin timer IRQ. Zero means the CPU did not publish a result.
#[cfg(feature = "qemu-ring3-smp-test")]
pub fn qemu_ring3_smp_test_result(cpu_index: usize) -> u64 {
    if cpu_index < RING3_SMP_MAX_CPUS {
        RING3_SMP_TEST_RESULTS[cpu_index].load(Ordering::Acquire)
    } else {
        0
    }
}

/// Start the per-CPU Ring 3 workload, release the user-mode barrier, and reclaim
/// the shared test address space only after every participant has returned.
#[cfg(feature = "qemu-ring3-smp-test")]
pub unsafe fn run_qemu_ring3_smp_test(
    plan: Ring3SmpTestPlan,
    allocator: &mut PhysicalFrameAllocator,
    bsp_cpu_index: usize,
    apic_ids: &[u8],
) -> Result<(), AddressSpaceError> {
    if bsp_cpu_index >= RING3_SMP_MAX_CPUS || apic_ids.len() + 1 > RING3_SMP_MAX_CPUS {
        return Err(AddressSpaceError::InvalidUserAddress);
    }
    // The second barrier word is the expected participant count.
    unsafe {
        core::ptr::write_volatile(
            (plan.barrier_frame + 8) as *mut u64,
            (apic_ids.len() + 1) as u64,
        );
    }
    RING3_SMP_TEST_START.store(true, Ordering::Release);
    run_qemu_ring3_smp_test_cpu(bsp_cpu_index);

    let expected = apic_ids.len() + 1;
    let mut completed = false;
    for _ in 0..1_000_000_000usize {
        if RING3_SMP_TEST_DONE.load(Ordering::Acquire) == expected {
            completed = true;
            break;
        }
        core::hint::spin_loop();
    }
    RING3_SMP_TEST_START.store(false, Ordering::Release);
    if !completed {
        // A CPU may still be executing in the shared page tables: do not reclaim them.
        core::mem::forget(plan.space);
        return Err(AddressSpaceError::Ring3TransitionFailed);
    }

    let mut all_success = RING3_SMP_TEST_RESULTS[bsp_cpu_index].load(Ordering::Acquire) == 1;
    for apic_id in apic_ids {
        all_success &= RING3_SMP_TEST_RESULTS[*apic_id as usize].load(Ordering::Acquire) == 1;
    }
    RING3_SMP_TEST_ROOT.store(0, Ordering::Release);
    unsafe { crate::console::debug_write(b"SYSTEMES_TEST: non-faulting CPU participants completed 10000-call syscall burst; count="); }
    let mut peer_digits = [0u8; 20];
    let peer_count = apic_ids.len();
    let peer_len = crate::console::encode_decimal(peer_count as u64, &mut peer_digits);
    unsafe { crate::console::debug_write(&peer_digits[..peer_len]); }
    unsafe { crate::console::debug_write(b"\r\n"); }
    // SAFETY: Every participant restored its original CR3 before incrementing DONE.
    unsafe { plan.space.destroy(allocator)? };
    unsafe { crate::console::debug_write(b"SYSTEMES_TEST: Ring 3 fault cleanup complete\r\n"); }
    if allocator.free_pages() != plan.free_pages_before {
        return Err(AddressSpaceError::FrameReclaimMismatch);
    }
    unsafe { crate::console::debug_write(b"SYSTEMES_TEST: frame allocator count restored; user stacks included\r\n"); }
    if !all_success {
        return Err(AddressSpaceError::SyscallReturnMismatch);
    }
    Ok(())
}

#[cfg(feature = "qemu-address-space-test")]
const KERNEL_SHARED_MAGIC: u64 = 0x5359_5354_454D_4553;
#[cfg(feature = "qemu-address-space-test")]
static KERNEL_SHARED_VALUE: u64 = KERNEL_SHARED_MAGIC;

/// Boot-time proof: create a private address space, map a user page, switch
/// CR3, exercise the user virtual address and verify the shared kernel map.
///
/// # Safety
///
/// Must run after the kernel page tables are active with interrupts disabled
/// or with the timer stopped, and must be called only once during boot.
#[cfg(feature = "qemu-address-space-test")]
pub unsafe fn run_qemu_address_space_test(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
) -> Result<(), AddressSpaceError> {
    const USER_MAGIC: u64 = 0x5553_4552_5041_4745;
    const USER_MAGIC_WRITTEN: u64 = 0x5752_4954_5445_4E21;

    let free_pages_before = allocator.free_pages();
    let user_frame = allocator
        .allocate_frame()
        .ok_or(AddressSpaceError::OutOfFrames)?;
    if user_frame % PAGE_SIZE != 0 {
        return Err(AddressSpaceError::InvalidFrameAlignment);
    }
    if user_frame >= IDENTITY_MAP_BYTES {
        return Err(AddressSpaceError::FrameOutsideIdentityWindow);
    }
    // SAFETY: `user_frame` is a fresh identity-accessible frame.
    unsafe { core::ptr::write_bytes(user_frame as *mut u8, 0, PAGE_SIZE as usize) };
    // SAFETY: The frame is writable through the kernel identity mapping.
    unsafe { core::ptr::write_volatile(user_frame as *mut u64, USER_MAGIC) };

    // SAFETY: The kernel root is active and the allocator owns free frames.
    let mut space = unsafe { AddressSpace::create(allocator, kernel_root)? };
    let virtual_address = USER_BASE + 0x20_0000;

    // SAFETY: The address space and allocator are valid for this mapping.
    unsafe {
        space.map_user_page(
            allocator,
            virtual_address,
            user_frame,
            UserPagePermissions::ReadWriteNoExecute,
        )?
    };
    // SAFETY: The mapping was just installed.
    unsafe {
        space.verify_user_mapping(
            virtual_address,
            user_frame,
            UserPagePermissions::ReadWriteNoExecute,
        )?
    };

    // Negative tests: duplicate and non-user mappings must be rejected.
    // SAFETY: The same valid address space is used; the second call must fail.
    if unsafe {
        space.map_user_page(
            allocator,
            virtual_address,
            user_frame,
            UserPagePermissions::ReadWriteNoExecute,
        )
    } != Err(AddressSpaceError::AlreadyMapped)
    {
        return Err(AddressSpaceError::DuplicateMappingAccepted);
    }
    // SAFETY: `USER_BASE - PAGE_SIZE` is aligned but outside the user window.
    if unsafe {
        space.map_user_page(
            allocator,
            USER_BASE - PAGE_SIZE,
            user_frame,
            UserPagePermissions::ReadWriteNoExecute,
        )
    } != Err(AddressSpaceError::InvalidUserAddress)
    {
        return Err(AddressSpaceError::InvalidMappingAccepted);
    }

    // SAFETY: The current CR3 points to the kernel root.
    let previous_root = unsafe { read_cr3() };
    // SAFETY: `space.root()` is a fully initialized PML4.
    unsafe { write_cr3(space.root()) };

    let mut result = Ok(());
    // SAFETY: Reading CR3 is permitted in ring 0.
    let active = unsafe { read_cr3() };
    if cr3_root(active) != space.root() {
        result = Err(AddressSpaceError::Cr3Mismatch);
    }

    if result.is_ok() {
        // The guard sets EFLAGS.AC only while SMAP is active, so ring-0 user
        // accesses below stay legal without weakening the rest of the kernel.
        let user_access = {
            let _smap_guard = crate::uaccess::UserAccessGuard::new();
            // SAFETY: The user page is identity-backed and present in this address space.
            let observed = unsafe { core::ptr::read_volatile(virtual_address as *const u64) };
            if observed != USER_MAGIC {
                Err(AddressSpaceError::UserReadMismatch)
            } else {
                // SAFETY: The page was mapped writable for the test.
                unsafe { core::ptr::write_volatile(virtual_address as *mut u64, USER_MAGIC_WRITTEN) };
                // SAFETY: Reading the backing frame proves the user write reached it.
                if unsafe { core::ptr::read_volatile(user_frame as *const u64) } != USER_MAGIC_WRITTEN {
                    Err(AddressSpaceError::UserWriteMismatch)
                } else {
                    Ok(())
                }
            }
        };
        result = user_access;
        if result.is_ok() {
            // SAFETY: KERNEL_SHARED_VALUE lives in the shared kernel mapping.
            if unsafe { core::ptr::read_volatile(&KERNEL_SHARED_VALUE) } != KERNEL_SHARED_MAGIC {
                result = Err(AddressSpaceError::KernelMappingMissing);
            }
        }
    }

    // SAFETY: Restore the original kernel root before reclaiming test frames.
    unsafe { write_cr3(previous_root) };
    // SAFETY: The kernel root is active, so none of the test process frames are in use.
    let reclaim_result = unsafe { space.destroy(allocator) };
    let frames_reclaimed = allocator.free_pages() == free_pages_before;
    reclaim_result?;
    if !frames_reclaimed {
        return Err(AddressSpaceError::FrameReclaimMismatch);
    }
    result
}

#[cfg(feature = "qemu-ring3-task-test")]
const RING3_TASK_COUNT: usize = 3;
/// P0-E: repeat create/exit/reap with fresh private spaces and scheduler slots.
#[cfg(feature = "qemu-ring3-task-test")]
const RING3_TASK_BURST_COUNT: usize = 3;
#[cfg(feature = "qemu-ring3-task-test")]
const RING3_TASK_EXIT_STATUS: u32 = 42;
#[cfg(feature = "qemu-ring3-task-test")]
const RING3_TASK_CODE_ADDRESSES: [u64; RING3_TASK_COUNT] =
    [USER_BASE + 0x2_0000, USER_BASE + 0x2_2000, USER_BASE + 0x2_4000];
#[cfg(feature = "qemu-ring3-task-test")]
const RING3_TASK_STACK_ADDRESSES: [u64; RING3_TASK_COUNT] =
    [USER_BASE + 0x2_1000, USER_BASE + 0x2_3000, USER_BASE + 0x2_5000];
#[cfg(feature = "qemu-ring3-task-test")]
const RING3_TASK_PAGE_FAULT_ADDRESS: u64 = USER_BASE + 0x9_0000;
#[cfg(feature = "qemu-ring3-task-test")]
const RING3_TASK_USER_CANARY: u8 = 0xAA;

#[cfg(feature = "qemu-ring3-task-test")]
static RING3_TASK_ROOTS: [AtomicU64; RING3_TASK_COUNT] =
    [const { AtomicU64::new(0) }; RING3_TASK_COUNT];
#[cfg(feature = "qemu-ring3-task-test")]
static RING3_TASK_RIPS: [AtomicU64; RING3_TASK_COUNT] =
    [const { AtomicU64::new(0) }; RING3_TASK_COUNT];
#[cfg(feature = "qemu-ring3-task-test")]
static RING3_TASK_RSPS: [AtomicU64; RING3_TASK_COUNT] =
    [const { AtomicU64::new(0) }; RING3_TASK_COUNT];
#[cfg(feature = "qemu-ring3-task-test")]
static RING3_TASK_OBSERVED_CR3: [AtomicU64; RING3_TASK_COUNT] =
    [const { AtomicU64::new(0) }; RING3_TASK_COUNT];
#[cfg(feature = "qemu-ring3-task-test")]
static RING3_TASK_EXECUTIONS: [AtomicUsize; RING3_TASK_COUNT] =
    [const { AtomicUsize::new(0) }; RING3_TASK_COUNT];
#[cfg(feature = "qemu-ring3-task-test")]
static RING3_TASK_OBSERVED_SYSCALL_STACKS: [AtomicU64; RING3_TASK_COUNT] =
    [const { AtomicU64::new(0) }; RING3_TASK_COUNT];
#[cfg(feature = "qemu-ring3-task-test")]
static RING3_TASK_OBSERVED_TSS_RSP0S: [AtomicU64; RING3_TASK_COUNT] =
    [const { AtomicU64::new(0) }; RING3_TASK_COUNT];
#[cfg(feature = "qemu-ring3-task-test")]
static RING3_TASK_OBSERVER_RAN: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "qemu-ring3-task-test")]
static RING3_TASK_PIDS: [AtomicU32; RING3_TASK_COUNT] =
    [const { AtomicU32::new(0) }; RING3_TASK_COUNT];
#[cfg(feature = "qemu-ring3-task-test")]
static RING3_TASK_WAVES_COMPLETED: AtomicUsize = AtomicUsize::new(0);

/// Entry of a schedulable Ring 3 task.
///
/// P0-C: this function installs the non-returning handoff flag and enters Ring 3.
/// A voluntary `SYS_EXIT` or a recoverable CPL3 `#PF`/`#GP` never returns to
/// this continuation: `systemes_task_exit_trampoline` switches to the stable
/// CPU-owned exit stack, publishes the first exit reason, and resumes another
/// task's IRETQ frame. Everything after `enter_user_mode` is therefore
/// unreachable for these tasks and only reports a protocol violation.
#[cfg(feature = "qemu-ring3-task-test")]
fn run_scheduled_ring3_task(index: usize) {
    RING3_TASK_EXECUTIONS[index].fetch_add(1, Ordering::AcqRel);
    let root = RING3_TASK_ROOTS[index].load(Ordering::Acquire);
    let user_rip = RING3_TASK_RIPS[index].load(Ordering::Acquire);
    let user_rsp = RING3_TASK_RSPS[index].load(Ordering::Acquire);
    let pid = RING3_TASK_PIDS[index].load(Ordering::Acquire);
    let cpu_index = crate::scheduler::current_cpu_index();
    // The process owns this task's address space and file table; activation
    // rejects a stale or cross-CPU binding before any user transition.
    let _ = crate::process::activate_process(cpu_index, pid);

    // SAFETY: The scheduler yields with interrupts enabled; user-mode entry
    // requires them masked, and this CPU owns the transition state.
    unsafe { core::arch::asm!("cli", options(nomem, nostack)) };
    let previous_root = unsafe { read_cr3() };
    // P0-D: publish the CPU/root relationship around the manual CR3 activation,
    // exactly as the scheduler does on ordinary context switches. The reaper
    // can then see an active CPU instead of relying on a stale TLB assumption.
    crate::process::mark_address_space_inactive(previous_root, cpu_index);
    // SAFETY: This Ring 3 task owns the private address space in `root`.
    unsafe { write_cr3(root) };
    crate::process::mark_address_space_active(root, cpu_index);
    RING3_TASK_OBSERVED_CR3[index]
        .store(cr3_root(unsafe { read_cr3() }), Ordering::Release);

    RING3_TASK_OBSERVED_SYSCALL_STACKS[index]
        .store(crate::interrupts::syscall_stack_top(), Ordering::Release);
    RING3_TASK_OBSERVED_TSS_RSP0S[index]
        .store(crate::interrupts::tss_rsp0(), Ordering::Release);
    crate::interrupts::enable_task_exit_handoff();

    // SAFETY: The private address space maps `user_rip` executable and
    // `user_rsp` writable, and the CPU descriptor tables are initialized.
    // A scheduler-owned task never returns from this call; the exit trampoline
    // restores the next task's CR3 directly.
    let _previous_root = previous_root;
    let status = unsafe { crate::interrupts::enter_user_mode(user_rip, user_rsp) };
    let _ = status;

    // Reaching this point means the exit protocol regressed to a synchronous
    // continuation. Record the violation instead of silently claiming success.
    crate::process::mark_address_space_inactive(root, cpu_index);
    unsafe { crate::console::debug_write(b"SYSTEMES_KERNEL_FAIL: task exit returned to terminating continuation\r\n") };
    let _ = crate::scheduler::mark_current_exited(crate::scheduler::TaskExitReason::VoluntaryExit {
        code: status as u32 as i32,
    });
    let _ = crate::process::mark_process_exited(cpu_index, pid, status as u32);
}

#[cfg(feature = "qemu-ring3-task-test")]
extern "C" fn ring3_task_entry_voluntary() {
    run_scheduled_ring3_task(0);
}

#[cfg(feature = "qemu-ring3-task-test")]
extern "C" fn ring3_task_entry_page_fault() {
    run_scheduled_ring3_task(1);
}

#[cfg(feature = "qemu-ring3-task-test")]
extern "C" fn ring3_task_entry_general_protection() {
    run_scheduled_ring3_task(2);
}

/// Independent kernel task that only succeeds once it observes every Ring 3
/// task in the terminal Exited state. Because it is scheduled after each Ring 3
/// termination, its completion is the proof that the CPU resumed another task.
#[cfg(feature = "qemu-ring3-task-test")]
extern "C" fn ring3_task_observer() {
    use crate::scheduler::{task_state, yield_now, TaskId, TaskState};

    for _ in 0..1_000_000usize {
        let all_exited = (0..RING3_TASK_COUNT).all(|index| {
            task_state(TaskId(index + 2)) == Some(TaskState::Exited)
        });
        if all_exited {
            RING3_TASK_OBSERVER_RAN.store(true, Ordering::Release);
            return;
        }
        yield_now();
    }
}

/// Boot-time proof: Ring 3 execution is a scheduler task. Three private address
/// spaces exit voluntarily, on #PF and on #GP; each termination publishes one
/// exit reason and the CPU schedules the observer or the next Ring 3 task.
///
/// # Safety
///
/// Must run once during boot with the kernel page tables active, the CPU
/// descriptor tables initialized, and no other CPU executing test tasks.
/// P0-D proof: exercise the remote IPI/ACK path used by deferred reaping.
///
/// The Ring 3 scenario is CPU-affine to the BSP, so no process ever activates
/// its CR3 on an AP. This check still sends a real shootdown IPI to every
/// online AP and requires each one to reload CR3 and publish the generation
/// ACK before the test declares the SMP protocol valid.
#[cfg(feature = "qemu-ring3-task-test")]
fn verify_remote_tlb_shootdown() -> Result<(), AddressSpaceError> {
    let bsp = crate::scheduler::current_cpu_index();
    let mut target_mask = 0u64;
    for cpu_index in 0..crate::interrupts::MAX_CPUS {
        if cpu_index != bsp && crate::smp::is_cpu_online(cpu_index) {
            target_mask |= 1u64 << cpu_index;
        }
    }
    if target_mask == 0 {
        return Ok(());
    }
    let acks_before = tlb_shootdown_ack_count();
    let stats = tlb_shootdown(0, target_mask);
    let mut acknowledged = false;
    for _ in 0..50_000_000u64 {
        let status = tlb_shootdown_status(target_mask, stats.generation);
        if status.targets != 0 && status.acks >= status.targets {
            acknowledged = true;
            break;
        }
        core::hint::spin_loop();
    }
    if !acknowledged
        || tlb_shootdown_ack_count() < acks_before + stats.targets as u64
    {
        return Err(AddressSpaceError::TlbShootdownTimeout);
    }
    unsafe {
        crate::console::debug_write(
            b"SYSTEMES_TEST: remote TLB shootdown acknowledged by all APs\r\n",
        );
    }
    Ok(())
}

#[cfg(feature = "qemu-ring3-task-test")]
pub unsafe fn run_qemu_ring3_task_exit_test(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
) -> Result<(), AddressSpaceError> {
    use crate::file_descriptors::ProcessFileTable;
    use crate::scheduler::{
        self, prepare_ring3_task, task_ring3_stack_tops,
        TaskExitReason, TaskId, TaskState,

    };

    let free_pages_before = allocator.free_pages();
    // Baselines are per wave: the P0-E burst reuses this function after the
    // scheduler and process table have been reset, so absolute counters must
    // not be confused with the three tasks of the current wave.
    let handoffs_before = crate::interrupts::task_exit_handoff_count();
    let stack_uses_before = crate::interrupts::task_exit_stack_use_count();
    let double_rejections_before = crate::interrupts::task_exit_double_rejection_count();
    let reaper_enqueued_before = crate::process::reaper_enqueued_count();
    let reaper_destroyed_before = crate::process::reaper_destroyed_count();
    // P0-C proof anchor: the exit stack must be CPU-owned and stable across
    // every task exit, never a task continuation stack.
    let exit_stack_top_before = crate::interrupts::task_exit_stack_top();
    for index in 0..RING3_TASK_COUNT {
        RING3_TASK_ROOTS[index].store(0, Ordering::Release);
        RING3_TASK_RIPS[index].store(0, Ordering::Release);
        RING3_TASK_RSPS[index].store(0, Ordering::Release);
        RING3_TASK_OBSERVED_CR3[index].store(0, Ordering::Release);
        RING3_TASK_OBSERVED_SYSCALL_STACKS[index].store(0, Ordering::Release);
        RING3_TASK_OBSERVED_TSS_RSP0S[index].store(0, Ordering::Release);
        RING3_TASK_EXECUTIONS[index].store(0, Ordering::Release);
        RING3_TASK_PIDS[index].store(0, Ordering::Release);
    }
    RING3_TASK_OBSERVER_RAN.store(false, Ordering::Release);

    let mut spaces: [Option<AddressSpace>; RING3_TASK_COUNT] = [None, None, None];
    let mut stack_frames = [0u64; RING3_TASK_COUNT];

    for index in 0..RING3_TASK_COUNT {
        let code_frame = allocator.allocate_frame().ok_or(AddressSpaceError::OutOfFrames)?;
        let stack_frame = allocator.allocate_frame().ok_or(AddressSpaceError::OutOfFrames)?;
        for frame in [code_frame, stack_frame] {
            if frame % PAGE_SIZE != 0 {
                return Err(AddressSpaceError::InvalidFrameAlignment);
            }
            if frame >= IDENTITY_MAP_BYTES {
                return Err(AddressSpaceError::FrameOutsideIdentityWindow);
            }
            // SAFETY: Both frames were just allocated and are identity accessible.
            unsafe { core::ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE as usize) };
        }
        stack_frames[index] = stack_frame;

        let mut code = [0u8; 32];
        let code_len = match index {
            0 => {
                // SYS_YIELD forces a context switch while this task is in user
                // mode; another Ring 3 task overwrites the CPU-local GS
                // continuation and CR3. SYS_EXIT(42) must still resume on this
                // task's own kernel stack and private page tables.
                code[..7].copy_from_slice(&[
                    0xB8, 0x04, 0x00, 0x00, 0x00, // mov eax, SYS_YIELD
                    0x0F, 0x05, // syscall
                ]);
                code[7..19].copy_from_slice(&[
                    0xB8, 0x03, 0x00, 0x00, 0x00, // mov eax, SYS_EXIT
                    0xBF, RING3_TASK_EXIT_STATUS as u8, 0x00, 0x00, 0x00, // mov edi, 42
                    0x0F, 0x05, // syscall
                ]);
                code[19..24].copy_from_slice(&[
                    0xC6, 0x44, 0x24, 0xF8, RING3_TASK_USER_CANARY, // mov [rsp-8], canary
                ]);
                code[24..26].copy_from_slice(&[0x0F, 0x0B]); // ud2
                26
            }
            1 => {
                // Read an unmapped user page: recoverable CPL3 #PF.
                code[..2].copy_from_slice(&[0x48, 0xB8]); // movabs rax, address
                code[2..10].copy_from_slice(&RING3_TASK_PAGE_FAULT_ADDRESS.to_le_bytes());
                code[10..13].copy_from_slice(&[0x48, 0x8B, 0x00]); // mov rax, [rax]
                code[13..18].copy_from_slice(&[
                    0xC6, 0x44, 0x24, 0xF8, RING3_TASK_USER_CANARY, // mov [rsp-8], canary
                ]);
                code[18..20].copy_from_slice(&[0x0F, 0x0B]); // ud2
                20
            }
            _ => {
                // HLT at CPL3 is a recoverable #GP.
                code[0] = 0xF4;
                code[1..6].copy_from_slice(&[
                    0xC6, 0x44, 0x24, 0xF8, RING3_TASK_USER_CANARY, // mov [rsp-8], canary
                ]);
                code[6..8].copy_from_slice(&[0x0F, 0x0B]); // ud2
                8
            }
        };
        // SAFETY: The code frame is freshly allocated and identity accessible.
        unsafe { core::ptr::copy_nonoverlapping(code.as_ptr(), code_frame as *mut u8, code_len) };

        // SAFETY: The kernel root is active and the allocator owns every frame.
        let mut space = unsafe { AddressSpace::create(allocator, kernel_root)? };
        // SAFETY: Addresses are page-aligned inside the user window and frames
        // are identity accessible.
        unsafe {
            space.map_user_page(
                allocator,
                RING3_TASK_CODE_ADDRESSES[index],
                code_frame,
                UserPagePermissions::ReadOnlyExecute,
            )?;
            space.map_user_page(
                allocator,
                RING3_TASK_STACK_ADDRESSES[index],
                stack_frame,
                UserPagePermissions::ReadWriteNoExecute,
            )?;
            space.verify_user_mapping(
                RING3_TASK_CODE_ADDRESSES[index],
                code_frame,
                UserPagePermissions::ReadOnlyExecute,
            )?;
            space.verify_user_mapping(
                RING3_TASK_STACK_ADDRESSES[index],
                stack_frame,
                UserPagePermissions::ReadWriteNoExecute,
            )?;
        }
        RING3_TASK_ROOTS[index].store(space.root(), Ordering::Release);
        RING3_TASK_RIPS[index]
            .store(RING3_TASK_CODE_ADDRESSES[index], Ordering::Release);
        RING3_TASK_RSPS[index].store(
            RING3_TASK_STACK_ADDRESSES[index] + PAGE_SIZE - 16,
            Ordering::Release,
        );
        spaces[index] = Some(space);
    }

    // Slot 0 stays boot, slot 1 is the kernel observer. Ring 3 slots are
    // reserved first, then published only after their initial kernel frame
    // exists. This exercises the same reservation path future processes use.
    // The observer is a CPU-owned kernel task: publish it already pinned so an
    // AP's idle steal can never race the affinity update.
    scheduler::initialize_cpu_pinned(
        crate::interrupts::bootstrap_cpu_index(),
        &[ring3_task_observer],
    )
    .map_err(|_| AddressSpaceError::Ring3TransitionFailed)?;
    let ring3_entries = [
        ring3_task_entry_voluntary as crate::scheduler::TaskEntry,
        ring3_task_entry_page_fault as crate::scheduler::TaskEntry,
        ring3_task_entry_general_protection as crate::scheduler::TaskEntry,
    ];
    for (index, (entry, space)) in ring3_entries
        .into_iter()
        .zip(spaces.into_iter().flatten())
        .enumerate()
    {
        let (pid, handle) = crate::process::create_process_with_task(
            space,
            RING3_TASK_CODE_ADDRESSES[index],
            RING3_TASK_STACK_ADDRESSES[index] + PAGE_SIZE - 16,
            ProcessFileTable::new(),
        )
        .map_err(|_| AddressSpaceError::Ring3TransitionFailed)?;
        RING3_TASK_PIDS[index].store(pid, Ordering::Release);
        let saved_rsp = prepare_ring3_task(handle, entry)
            .map_err(|_| AddressSpaceError::Ring3TransitionFailed)?;
        crate::process::publish_process_task(pid, saved_rsp)
            .map_err(|_| AddressSpaceError::Ring3TransitionFailed)?;
    }

    // Boot task drives the round-robin scheduler; no timer preemption is enabled,
    // so each Ring 3 task runs to its termination path before the next switch.
    for _ in 0..1_000_000usize {
        let ring3_exited = (0..RING3_TASK_COUNT).all(|index| {
            scheduler::task_state(TaskId(index + 2)) == Some(TaskState::Exited)
        });
        // The observer must itself be scheduled and reach Exited: that proves the
        // CPU resumed an independent task after the last Ring 3 termination.
        let observer_exited =
            scheduler::task_state(TaskId(1)) == Some(TaskState::Exited);
        if ring3_exited && observer_exited {
            break;
        }
        scheduler::yield_now();
    }

    let mut failure = Ok(());
    let expected_reasons = [
        TaskExitReason::VoluntaryExit {
            code: RING3_TASK_EXIT_STATUS as i32,
        },
        TaskExitReason::PageFault {
            cr2: RING3_TASK_PAGE_FAULT_ADDRESS as usize,
            error_code: 0x4,
            rip: (RING3_TASK_CODE_ADDRESSES[1] + 10) as usize,
        },
        TaskExitReason::GeneralProtection {
            error_code: 0,
            rip: RING3_TASK_CODE_ADDRESSES[2] as usize,
        },
    ];
    for index in 0..RING3_TASK_COUNT {
        let id = TaskId(index + 2);
        if scheduler::task_state(id) != Some(TaskState::Exited) {
            failure = Err(AddressSpaceError::Ring3TaskStateMismatch);
            break;
        }
        if scheduler::task_exit_reason(id) != Some(expected_reasons[index]) {
            failure = Err(AddressSpaceError::Ring3TaskExitReasonMismatch);
            break;
        }
        if RING3_TASK_EXECUTIONS[index].load(Ordering::Acquire) != 1 {
            failure = Err(AddressSpaceError::Ring3TaskResumedInUserMode);
            break;
        }
        let expected_root = RING3_TASK_ROOTS[index].load(Ordering::Acquire);
        if RING3_TASK_OBSERVED_CR3[index].load(Ordering::Acquire) != expected_root {
            failure = Err(AddressSpaceError::Ring3TaskCr3Mismatch);
            break;
        }
        let Some((expected_tss_rsp0, expected_syscall_stack)) = task_ring3_stack_tops(id) else {
            failure = Err(AddressSpaceError::Ring3TaskStackMismatch);
            break;
        };
        if RING3_TASK_OBSERVED_SYSCALL_STACKS[index].load(Ordering::Acquire) != expected_syscall_stack
            || RING3_TASK_OBSERVED_TSS_RSP0S[index].load(Ordering::Acquire) != expected_tss_rsp0
        {
            failure = Err(AddressSpaceError::Ring3TaskStackMismatch);
            break;
        }
        // The user canary is only written by instructions after the exit path;
        // a non-zero value means kernel code resumed user mode.
        let canary = unsafe {
            let _guard = crate::uaccess::UserAccessGuard::new();
            core::ptr::read_volatile(
                (stack_frames[index] as *const u8).add(PAGE_SIZE as usize - 24),
            )
        };
        if canary != 0 {
            failure = Err(AddressSpaceError::Ring3TaskResumedInUserMode);
            break;
        }
        let pid = RING3_TASK_PIDS[index].load(Ordering::Acquire);
        if !matches!(
            crate::process::process_state(pid),
            Some(crate::process::ProcessState::Exited)
                | Some(crate::process::ProcessState::Reaping)
        ) {
            failure = Err(AddressSpaceError::Ring3TaskStateMismatch);
            break;
        }
    }
    if failure.is_ok() && !RING3_TASK_OBSERVER_RAN.load(Ordering::Acquire) {
        failure = Err(AddressSpaceError::Ring3TaskObserverMissing);
    }
    if failure.is_ok() && scheduler::current_task() != TaskId(0) {
        failure = Err(AddressSpaceError::Ring3TaskStateMismatch);
    }
    // P0-C: every termination must have used the non-returning handoff and the
    // same stable CPU exit stack; a synchronous continuation return would leave
    // these counters at zero.
    if failure.is_ok()
        && (crate::interrupts::task_exit_handoff_count()
            != handoffs_before + RING3_TASK_COUNT as u64
            || crate::interrupts::task_exit_stack_use_count()
                != stack_uses_before + RING3_TASK_COUNT as u64
            || crate::interrupts::task_exit_double_rejection_count()
                != double_rejections_before + RING3_TASK_COUNT as u64
            || exit_stack_top_before == 0
            || crate::interrupts::task_exit_stack_top() != exit_stack_top_before)
    {
        failure = Err(AddressSpaceError::Ring3TaskStateMismatch);
    }

    // P0-D: no exit path destroys directly. The CPU-affine reaper drains the
    // queued processes only after pins and active CPU masks are clear.
    let cpu_index = crate::scheduler::current_cpu_index();
    let reaped = crate::process::drain_reaper(cpu_index, allocator)
        .map_err(|_| AddressSpaceError::FrameReclaimMismatch)?;
    if reaped != RING3_TASK_COUNT
        || crate::process::reaper_pending_count() != 0
        || crate::process::reaper_enqueued_count()
            != reaper_enqueued_before + RING3_TASK_COUNT as u64
        || crate::process::reaper_destroyed_count()
            != reaper_destroyed_before + RING3_TASK_COUNT as u64
    {
        failure = Err(AddressSpaceError::Ring3TaskStateMismatch);
    }
    for index in 0..RING3_TASK_COUNT {
        let pid = RING3_TASK_PIDS[index].load(Ordering::Acquire);
        if crate::process::process_state(pid).is_some() {
            failure = Err(AddressSpaceError::Ring3TaskStateMismatch);
        }
    }
    if allocator.free_pages() != free_pages_before {
        return Err(AddressSpaceError::FrameReclaimMismatch);
    }

    failure?;

    unsafe {
        crate::console::debug_write(
            b"SYSTEMES_TEST: Ring 3 task exited voluntarily and published code=42\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: Ring 3 task terminated on page fault with CR2 and RIP\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: Ring 3 task terminated on general protection fault\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: Ring 3 observer resumed after every task reached Exited\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: per-task CR3 and non-returning exit verified count=3\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: non-returning exit handoff used count=3\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: CPU-owned exit stack stable across task exits\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: reaper drained queued processes count=3\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: address spaces destroyed after deferred reaping\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: competing termination rejected without changing first reason\r\n",
        );
    }

    // P0-E: run the same create/exit/reap burst repeatedly with fresh private
    // spaces. A leak or a stale scheduler slot breaks the next wave's CSS for
    // exactly this reason.
    let completed = RING3_TASK_WAVES_COMPLETED.fetch_add(1, Ordering::AcqRel) + 1;
    if completed < RING3_TASK_BURST_COUNT {
        if !scheduler::reset_after_all_tasks_exit() {
            unsafe {
                crate::console::debug_write(
                    b"SYSTEMES_KERNEL_FAIL: scheduler reset between exit bursts failed\r\n",
                );
            }
            return Err(AddressSpaceError::Ring3TaskStateMismatch);
        }
        return unsafe { run_qemu_ring3_task_exit_test(allocator, kernel_root) };
    }

    // P0-D: the non-CPU-affine part of the protocol (IPI, CR3 reload, ACK) is
    // only observable when at least one AP is online.
    verify_remote_tlb_shootdown()?;

    unsafe {
        crate::console::debug_write(
            b"SYSTEMES_TEST: repeated create/exit/reap bursts completed count=3\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: multiple runnable Ring 3 tasks per CPU verified\r\n",
        );
        crate::console::debug_write(
            b"SYSTEMES_TEST: Ring 3 task exit matrix complete\r\n",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_space_tracks_each_owned_frame_once() {
        let mut space = AddressSpace {
            root: 0,
            owned_frames: Vec::new(),
        };
        assert_eq!(space.record_owned_frame(0x1000), Ok(()));
        assert_eq!(space.record_owned_frame(0x2000), Ok(()));
        assert_eq!(space.record_owned_frame(0x1000), Ok(()));
        assert_eq!(space.owned_frames.len(), 2);
        for index in 2..260 {
            assert_eq!(
                space.record_owned_frame((index as u64 + 1) * PAGE_SIZE),
                Ok(())
            );
        }
        assert_eq!(space.owned_frames.len(), 260);
    }

    #[test]
    fn user_window_starts_at_pml4_slot_one() {
        assert!(!is_user_address(USER_BASE - PAGE_SIZE));
        assert!(is_user_address(USER_BASE));
        assert!(is_user_address(USER_LIMIT - PAGE_SIZE));
        assert!(!is_user_address(USER_LIMIT));
        assert_eq!(page_table_indices(USER_BASE), [1, 0, 0, 0]);
    }

    #[test]
    fn user_permissions_never_allow_write_execute() {
        for permission in [
            UserPagePermissions::ReadOnlyExecute,
            UserPagePermissions::ReadWriteNoExecute,
        ] {
            let flags = permission.flags();
            assert_ne!(flags & PRESENT, 0);
            assert_ne!(flags & USER, 0);
            assert!(!(flags & WRITABLE != 0 && flags & NX == 0));
        }
        assert_eq!(
            UserPagePermissions::ReadOnlyExecute.flags() & (WRITABLE | NX),
            0
        );
        assert_eq!(
            UserPagePermissions::ReadWriteNoExecute.flags() & NX,
            NX
        );
    }

    #[test]
    fn rejects_unaligned_out_of_range_and_unreachable_physical_pages() {
        assert_eq!(
            validate_user_mapping(USER_BASE + 1, PAGE_SIZE),
            Err(AddressSpaceError::UnalignedAddress)
        );
        assert_eq!(
            validate_user_mapping(USER_BASE, PAGE_SIZE + 1),
            Err(AddressSpaceError::UnalignedAddress)
        );
        assert_eq!(
            validate_user_mapping(USER_BASE - PAGE_SIZE, PAGE_SIZE),
            Err(AddressSpaceError::InvalidUserAddress)
        );
        assert_eq!(
            validate_user_mapping(USER_LIMIT, PAGE_SIZE),
            Err(AddressSpaceError::InvalidUserAddress)
        );
        assert_eq!(
            validate_user_mapping(USER_BASE, IDENTITY_MAP_BYTES),
            Err(AddressSpaceError::FrameOutsideIdentityWindow)
        );
        assert_eq!(
            validate_user_mapping(USER_BASE, PAGE_SIZE),
            Ok([1, 0, 0, 0])
        );
    }

    #[test]
    fn cr3_root_masks_control_bits() {
        let root = 0x0000_0001_2345_6000;
        assert_eq!(cr3_root(root | 0xFFF), root);
        assert_eq!(cr3_root(0), 0);
    }
}
