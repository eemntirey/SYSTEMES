//! Private process page tables sharing the kernel identity mapping.
//!
//! User virtual addresses live in PML4 slot 1 (512 GiB..128 TiB). Every
//! address space copies the kernel PML4 slot 0, so kernel code, stacks and
//! device mappings stay visible after a CR3 switch. User leaves always carry
//! the U/S bit; writable-executable user pages are not representable.

#[cfg(feature = "qemu-ring3-test")]
use crate::interrupts::RING3_TEST_MAGIC;
use crate::memory::{PhysicalFrameAllocator, PAGE_SIZE};
use crate::paging::{page_table_indices, IDENTITY_MAP_BYTES};

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
}

#[cfg(feature = "qemu-ring3-test")]
const RING3_CODE_ADDRESS: u64 = USER_BASE + 0x4000;
#[cfg(feature = "qemu-ring3-test")]
const RING3_STACK_ADDRESS: u64 = USER_BASE + 0x5000;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserPagePermissions {
    ReadOnlyExecute,
    ReadWriteNoExecute,
}

impl UserPagePermissions {
    const fn flags(self) -> u64 {
        match self {
            Self::ReadOnlyExecute => PRESENT | USER,
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

/// A process-owned PML4. Page-table frames are not reclaimed yet; the boot
/// self-test creates a single address space and documents this limit.
pub struct AddressSpace {
    root: u64,
}

impl AddressSpace {
    pub const fn root(&self) -> u64 {
        self.root
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
        let root = allocator
            .allocate_frame()
            .ok_or(AddressSpaceError::OutOfFrames)?;
        if root % PAGE_SIZE != 0 {
            return Err(AddressSpaceError::InvalidFrameAlignment);
        }
        if root >= IDENTITY_MAP_BYTES {
            return Err(AddressSpaceError::FrameOutsideIdentityWindow);
        }

        // SAFETY: kernel_root points to a valid PML4 under the active identity map.
        let kernel_entry_zero = unsafe { (kernel_root as *const u64).read() };
        if kernel_entry_zero & PRESENT == 0 {
            return Err(AddressSpaceError::KernelRootNotMapped);
        }

        // SAFETY: root is a freshly allocated, page-aligned frame.
        unsafe { core::ptr::write_bytes(root as *mut u8, 0, PAGE_SIZE as usize) };
        // SAFETY: A PML4 has 512 entries; slot 0 shares the kernel identity map.
        unsafe { (root as *mut u64).write(kernel_entry_zero) };

        Ok(Self { root })
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
        let pdpt = unsafe { next_table(allocator, pml4.add(indices[0]))? };
        // SAFETY: `pdpt` is a valid table allocated above.
        let pd = unsafe { next_table(allocator, pdpt.add(indices[1]))? };
        // SAFETY: `pd` is a valid table allocated above.
        let pt = unsafe { next_table(allocator, pd.add(indices[2]))? };
        // SAFETY: `pt` is a valid leaf table allocated above.
        let pte = unsafe { pt.add(indices[3]) };
        // SAFETY: The leaf entry is readable and belongs to `pt`.
        if unsafe { pte.read() } & PRESENT != 0 {
            return Err(AddressSpaceError::AlreadyMapped);
        }
        // SAFETY: The leaf entry is not present and the frame was validated.
        unsafe { pte.write((physical_frame & ADDRESS_MASK) | permissions.flags()) };
        Ok(())
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

unsafe fn next_table(
    allocator: &mut PhysicalFrameAllocator,
    entry_ptr: *mut u64,
) -> Result<*mut u64, AddressSpaceError> {
    // SAFETY: `entry_ptr` belongs to a page table owned by the caller.
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
    if frame % PAGE_SIZE != 0 {
        return Err(AddressSpaceError::InvalidFrameAlignment);
    }
    if frame >= IDENTITY_MAP_BYTES {
        return Err(AddressSpaceError::FrameOutsideIdentityWindow);
    }
    // SAFETY: `frame` is a freshly allocated, identity-accessible 4 KiB frame.
    unsafe { core::ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE as usize) };
    // SAFETY: The parent entry is not present and accepts the new table.
    unsafe { entry_ptr.write(frame | PRESENT | WRITABLE | USER) };
    Ok(frame as *mut u64)
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

    // SAFETY: Restore the original address space before resuming normal boot.
    unsafe { write_cr3(previous_root) };
    if previous_flags & (1 << 9) != 0 {
        // SAFETY: The previous root is restored and the caller had interrupts enabled.
        unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
    }
    transition_result
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

    // SAFETY: Restore the original kernel root before returning.
    unsafe { write_cr3(previous_root) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

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
