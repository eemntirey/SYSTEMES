//! Supervisor access protection (SMEP/SMAP) and scoped user-memory access.
//!
//! SMEP prevents ring-0 instruction fetches from user pages. SMAP prevents
//! ring-0 data accesses to user pages unless EFLAGS.AC is set, which the
//! `stac`/`clac` helpers below toggle for tightly scoped copies. Both features
//! are enabled only when CPUID reports them; unsupported CPUs keep running
//! without them and the boot marker reports the real state.

use crate::address_space::{USER_BASE, USER_LIMIT};
use core::arch::asm;
use core::arch::x86_64::{__cpuid, __cpuid_count};

#[cfg(any(feature = "qemu-smap-violation-test", feature = "qemu-smep-violation-test"))]
use crate::address_space::{
    write_cr3, AddressSpace, AddressSpaceError, UserPagePermissions,
};
#[cfg(any(feature = "qemu-smap-violation-test", feature = "qemu-smep-violation-test"))]
use crate::console::debug_write;
#[cfg(any(feature = "qemu-smap-violation-test", feature = "qemu-smep-violation-test"))]
use crate::memory::{PhysicalFrameAllocator, PAGE_SIZE};
#[cfg(any(feature = "qemu-smap-violation-test", feature = "qemu-smep-violation-test"))]
use crate::paging::IDENTITY_MAP_BYTES;

pub const CR4_SMEP: u64 = 1 << 20;
pub const CR4_SMAP: u64 = 1 << 21;
const CPUID_LEAF7_EBX_SMEP: u32 = 1 << 7;
const CPUID_LEAF7_EBX_SMAP: u32 = 1 << 20;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SupervisorFeatures {
    pub smep: bool,
    pub smap: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectionError {
    SmepDidNotLatch,
    SmapDidNotLatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UaccessError {
    NotUserAddress,
}

/// Decode CPUID leaf 7 subleaf 0 EBX into supervisor protection features.
pub const fn features_from_leaf7_ebx(ebx: u32) -> SupervisorFeatures {
    SupervisorFeatures {
        smep: ebx & CPUID_LEAF7_EBX_SMEP != 0,
        smap: ebx & CPUID_LEAF7_EBX_SMAP != 0,
    }
}

/// Add the requested CR4 bits without touching unrelated control bits.
pub const fn cr4_with_features(current: u64, features: SupervisorFeatures) -> u64 {
    let mut value = current;
    if features.smep {
        value |= CR4_SMEP;
    }
    if features.smap {
        value |= CR4_SMAP;
    }
    value
}

/// Return true only when `[address, address + size)` stays inside the user window.
pub const fn is_user_range(address: u64, size: u64) -> bool {
    if size == 0 {
        return false;
    }
    match address.checked_add(size) {
        Some(end) => address >= USER_BASE && end <= USER_LIMIT,
        None => false,
    }
}

/// Validate an aligned 8-byte user pointer before any dereference.
pub const fn validate_user_u64(address: u64) -> Result<(), UaccessError> {
    if address % 8 != 0 || !is_user_range(address, 8) {
        Err(UaccessError::NotUserAddress)
    } else {
        Ok(())
    }
}

/// Detect SMEP/SMAP support through CPUID leaf 7.
pub fn detect() -> SupervisorFeatures {
    // CPUID leaf 0 is available on every x86_64 CPU.
    let max_leaf = __cpuid(0).eax;
    if max_leaf < 7 {
        return SupervisorFeatures::default();
    }
    // Leaf 7 is supported by the check above.
    features_from_leaf7_ebx(__cpuid_count(7, 0).ebx)
}

/// Read CR4 from ring 0.
pub fn read_cr4() -> u64 {
    let value: u64;
    // SAFETY: Reading CR4 is permitted in ring 0.
    unsafe { asm!("mov {}, cr4", out(reg) value, options(nomem, nostack, preserves_flags)) };
    value
}

/// Read the currently active SMEP/SMAP state from CR4.
pub fn active_features() -> SupervisorFeatures {
    let cr4 = read_cr4();
    SupervisorFeatures {
        smep: cr4 & CR4_SMEP != 0,
        smap: cr4 & CR4_SMAP != 0,
    }
}

/// Enable every detected feature and fail closed if a supported bit does not latch.
pub fn enable() -> Result<SupervisorFeatures, ProtectionError> {
    let detected = detect();
    let current = read_cr4();
    let desired = cr4_with_features(current, detected);
    if desired != current {
        // SAFETY: CPUID reported every feature whose CR4 bit is being set.
        unsafe {
            asm!("mov cr4, {}", in(reg) desired, options(nostack, preserves_flags));
        }
    }
    let active = active_features();
    if detected.smep && !active.smep {
        return Err(ProtectionError::SmepDidNotLatch);
    }
    if detected.smap && !active.smap {
        return Err(ProtectionError::SmapDidNotLatch);
    }
    Ok(active)
}

/// Set EFLAGS.AC so SMAP permits ring-0 access to user pages.
///
/// # Safety
///
/// The caller must scope this to deliberate user-memory accesses and must not
/// call it on a CPU without SMAP support.
pub unsafe fn stac() {
    // SAFETY: The caller guarantees SMAP support and a scoped user access.
    unsafe { asm!("stac", options(nostack, preserves_flags)) };
}

/// Clear EFLAGS.AC to restore SMAP enforcement.
///
/// # Safety
///
/// Must be called on a CPU with SMAP support after a matching `stac`.
pub unsafe fn clac() {
    // SAFETY: The caller guarantees SMAP support and a matching `stac`.
    unsafe { asm!("clac", options(nostack, preserves_flags)) };
}

/// Scoped SMAP bypass. When SMAP is inactive this guard is a no-op.
pub struct UserAccessGuard {
    smap_active: bool,
}

impl UserAccessGuard {
    pub fn new() -> Self {
        let smap_active = active_features().smap;
        if smap_active {
            // SAFETY: SMAP is active, so STAC is architecturally available.
            unsafe { stac() };
        }
        Self { smap_active }
    }
}

impl Drop for UserAccessGuard {
    fn drop(&mut self) {
        if self.smap_active {
            // SAFETY: This closes the matching STAC issued by `new`.
            unsafe { clac() };
        }
    }
}

/// Read one aligned `u64` from the user window with a scoped SMAP bypass.
///
/// # Safety
///
/// The caller must ensure the address is mapped for read in the active address
/// space. An unmapped page raises a fatal page fault until recoverable faults
/// are implemented.
pub unsafe fn read_user_u64(address: u64) -> Result<u64, UaccessError> {
    validate_user_u64(address)?;
    let _guard = UserAccessGuard::new();
    // SAFETY: The caller guarantees the user mapping; the guard scopes SMAP.
    Ok(unsafe { core::ptr::read_volatile(address as *const u64) })
}

/// Write one aligned `u64` into the user window with a scoped SMAP bypass.
///
/// # Safety
///
/// The caller must ensure the address is mapped writable in the active address
/// space. An unmapped page raises a fatal page fault until recoverable faults
/// are implemented.
pub unsafe fn write_user_u64(address: u64, value: u64) -> Result<(), UaccessError> {
    validate_user_u64(address)?;
    let _guard = UserAccessGuard::new();
    // SAFETY: The caller guarantees the user mapping; the guard scopes SMAP.
    unsafe { core::ptr::write_volatile(address as *mut u64, value) };
    Ok(())
}

/// Map one user page in a private address space and activate it.
#[cfg(any(feature = "qemu-smap-violation-test", feature = "qemu-smep-violation-test"))]
unsafe fn prepare_user_page(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
    offset: u64,
    permissions: UserPagePermissions,
) -> Result<(u64, u64), AddressSpaceError> {
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

    // SAFETY: The allocator and kernel root are valid during single-core boot.
    let mut space = unsafe { AddressSpace::create(allocator, kernel_root)? };
    let virtual_address = USER_BASE + offset;
    // SAFETY: `map_user_page` validates alignment, window and physical frame.
    unsafe { space.map_user_page(allocator, virtual_address, user_frame, permissions)? };
    // SAFETY: `space.root()` is a fully initialized PML4 sharing the kernel map.
    unsafe { write_cr3(space.root()) };
    Ok((user_frame, virtual_address))
}

/// Deliberately read a user page from ring 0 without STAC. With SMAP active the
/// read must fault with #PF(P=1, supervisor access, W/R=0), error code 0x1.
///
/// # Safety
///
/// Boot-only test that never returns when SMAP behaves correctly.
#[cfg(feature = "qemu-smap-violation-test")]
pub unsafe fn run_qemu_smap_violation_test(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
) -> Result<(), AddressSpaceError> {
    if !active_features().smap {
        return Err(AddressSpaceError::SmapUnavailable);
    }
    // SAFETY: Boot-time allocator and kernel root; the mapping is test-only.
    let (_user_frame, virtual_address) = unsafe {
        prepare_user_page(
            allocator,
            kernel_root,
            0x30_0000,
            UserPagePermissions::ReadWriteNoExecute,
        )?
    };
    // SAFETY: Port 0xE9 is the boot debug console.
    unsafe { debug_write(b"SYSTEMES_TEST: attempting unguarded supervisor read of a user page\r\n") };
    // SAFETY: This deliberate unguarded read must fault while SMAP is active.
    let observed = unsafe { core::ptr::read_volatile(virtual_address as *const u64) };
    let _ = observed;
    // SAFETY: Port 0xE9 is the boot debug console.
    unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: SMAP did not block supervisor read\r\n") };
    Err(AddressSpaceError::SmapDidNotBlock)
}

/// Deliberately fetch instructions from a user page from ring 0. With SMEP
/// active the call must fault with #PF(P=1, supervisor access, I/D=1), error code 0x11.
///
/// # Safety
///
/// Boot-only test that never returns when SMEP behaves correctly.
#[cfg(feature = "qemu-smep-violation-test")]
pub unsafe fn run_qemu_smep_violation_test(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
) -> Result<(), AddressSpaceError> {
    if !active_features().smep {
        return Err(AddressSpaceError::SmepUnavailable);
    }
    // SAFETY: Boot-time allocator and kernel root; the mapping is test-only.
    let (user_frame, virtual_address) = unsafe {
        prepare_user_page(
            allocator,
            kernel_root,
            0x40_0000,
            UserPagePermissions::ReadOnlyExecute,
        )?
    };
    // SAFETY: The backing frame is identity mapped; 0xC3 is the RET opcode.
    unsafe { core::ptr::write_volatile(user_frame as *mut u8, 0xC3) };
    // SAFETY: Port 0xE9 is the boot debug console.
    unsafe { debug_write(b"SYSTEMES_TEST: attempting supervisor execution of a user page\r\n") };
    let entry: extern "C" fn() = unsafe { core::mem::transmute(virtual_address as usize) };
    // SAFETY: With SMEP active this instruction fetch must fault before executing.
    entry();
    // SAFETY: Port 0xE9 is the boot debug console.
    unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: SMEP did not block supervisor execution\r\n") };
    Err(AddressSpaceError::SmepDidNotBlock)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_only_the_smep_and_smap_cpuid_bits() {
        assert_eq!(
            features_from_leaf7_ebx(0),
            SupervisorFeatures {
                smep: false,
                smap: false
            }
        );
        assert_eq!(
            features_from_leaf7_ebx((1 << 7) | (1 << 20)),
            SupervisorFeatures {
                smep: true,
                smap: true
            }
        );
        assert_eq!(
            features_from_leaf7_ebx(1 << 7),
            SupervisorFeatures {
                smep: true,
                smap: false
            }
        );
        assert_eq!(
            features_from_leaf7_ebx(1 << 20),
            SupervisorFeatures {
                smep: false,
                smap: true
            }
        );
        assert_eq!(
            features_from_leaf7_ebx(1 << 8),
            SupervisorFeatures {
                smep: false,
                smap: false
            }
        );
    }

    #[test]
    fn cr4_keeps_unrelated_bits_and_sets_the_exact_positions() {
        let base = 0x10;
        assert_eq!(
            cr4_with_features(
                base,
                SupervisorFeatures {
                    smep: false,
                    smap: false
                }
            ),
            base
        );
        assert_eq!(
            cr4_with_features(
                base,
                SupervisorFeatures {
                    smep: true,
                    smap: false
                }
            ),
            base | CR4_SMEP
        );
        assert_eq!(
            cr4_with_features(
                base,
                SupervisorFeatures {
                    smep: false,
                    smap: true
                }
            ),
            base | CR4_SMAP
        );
        assert_eq!(
            cr4_with_features(
                base,
                SupervisorFeatures {
                    smep: true,
                    smap: true
                }
            ),
            base | CR4_SMEP | CR4_SMAP
        );
        assert_eq!(CR4_SMEP, 1 << 20);
        assert_eq!(CR4_SMAP, 1 << 21);
    }

    #[test]
    fn user_ranges_fit_exactly_inside_the_user_window() {
        assert!(is_user_range(USER_BASE, 8));
        assert!(is_user_range(USER_LIMIT - 8, 8));
        assert!(!is_user_range(USER_BASE - 8, 8));
        assert!(!is_user_range(USER_LIMIT, 8));
        assert!(!is_user_range(USER_LIMIT - 4, 8));
        assert!(!is_user_range(USER_BASE, 0));
        assert!(!is_user_range(u64::MAX, 8));
    }

    #[test]
    fn user_u64_pointers_must_be_aligned_and_inside_the_window() {
        assert_eq!(validate_user_u64(USER_BASE), Ok(()));
        assert_eq!(
            validate_user_u64(USER_BASE + 1),
            Err(UaccessError::NotUserAddress)
        );
        assert_eq!(
            validate_user_u64(USER_BASE - 8),
            Err(UaccessError::NotUserAddress)
        );
        assert_eq!(validate_user_u64(USER_LIMIT - 8), Ok(()));
        assert_eq!(
            validate_user_u64(USER_LIMIT - 4),
            Err(UaccessError::NotUserAddress)
        );
    }
}
