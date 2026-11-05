//! x87/SSE task-context support for x86_64.
//!
//! The kernel is built for `x86_64-unknown-none`, which disables SSE code
//! generation (`+soft-float`). The compiler therefore never keeps live values in
//! FPU or SIMD registers across Rust statements, so a task's x87/SSE state can be
//! captured with a single `FXSAVE64` image at interrupt entry and reloaded with
//! `FXRSTOR64` before `IRETQ`.
//!
//! `FXSAVE64` saves the x87 control/status/tag words, the eight 80-bit x87 data
//! registers, MXCSR and all sixteen XMM registers. AVX/AVX-512 upper state
//! (YMM/ZMM, opmask) is **not** covered yet; that requires XSAVE with a
//! configured XCR0 and is tracked as a later kernel task.

use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Size of one FXSAVE image in bytes.
pub const FXSAVE_AREA_SIZE: usize = 512;
/// Offset of the x87 control word inside a FXSAVE image.
pub const X87_CONTROL_WORD_OFFSET: usize = 0;
/// Offset of the abridged x87 tag word inside a FXSAVE image.
pub const X87_TAG_WORD_OFFSET: usize = 4;
/// Offset of MXCSR inside a FXSAVE image.
pub const MXCSR_OFFSET: usize = 24;
/// MXCSR value after `FNINIT` plus default exception masks.
pub const MXCSR_DEFAULT: u32 = 0x1F80;
/// x87 control word after `FNINIT` (all exceptions masked, extended precision).
pub const X87_CONTROL_WORD_DEFAULT: u16 = 0x037F;
/// Abridged x87 tag word with an empty register stack.
pub const X87_TAG_WORD_EMPTY: u8 = 0xFF;

const CR0_MP: u64 = 1 << 1;
const CR0_EM: u64 = 1 << 2;
const CR0_TS: u64 = 1 << 3;
const CR0_NE: u64 = 1 << 5;
const CR4_OSFXSR: u64 = 1 << 9;
const CR4_OSXMMEXCPT: u64 = 1 << 10;
const REQUIRED_CR0_BITS: u64 = CR0_MP | CR0_NE;
const FORBIDDEN_CR0_BITS: u64 = CR0_EM | CR0_TS;
const REQUIRED_CR4_BITS: u64 = CR4_OSFXSR | CR4_OSXMMEXCPT;

static DEFAULT_MXCSR_VALUE: u32 = MXCSR_DEFAULT;
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Errors returned by [`initialize`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FpuError {
    /// CR0/CR4 did not latch the FPU/SSE enable bits.
    ControlRegisterReadBackFailed,
    /// The CPU did not preserve an FXSAVE image across FXRSTOR.
    RoundTripFailed,
    /// The captured initial image does not match the documented layout.
    InvalidInitialImage,
}

/// A 16-byte-aligned 512-byte FXSAVE64 image.
///
/// `FXSAVE64` requires 16-byte alignment; the alignment is part of the type so
/// static task tables and local scratch areas cannot violate it.
#[repr(C, align(16))]
pub struct FpuArea {
    bytes: [u8; FXSAVE_AREA_SIZE],
}

impl FpuArea {
    /// Create a zeroed image. Zeroes are not a valid processor state; call
    /// [`capture`] or copy a captured template before restoring an area.
    pub const fn zeroed() -> Self {
        Self { bytes: [0; FXSAVE_AREA_SIZE] }
    }

    /// Address of the image, used by the IRQ assembly path.
    pub fn address(&self) -> usize {
        self as *const Self as usize
    }

    /// Mutable address of the image.
    pub fn address_mut(&mut self) -> usize {
        self as *mut Self as usize
    }

    /// Addressable image bytes.
    pub fn as_bytes(&self) -> &[u8; FXSAVE_AREA_SIZE] {
        &self.bytes
    }

    /// x87 control word saved at offset 0.
    pub fn x87_control_word(&self) -> u16 {
        u16::from_le_bytes([self.bytes[X87_CONTROL_WORD_OFFSET], self.bytes[X87_CONTROL_WORD_OFFSET + 1]])
    }

    /// Abridged x87 tag word saved at offset 4.
    pub fn x87_tag_word(&self) -> u8 {
        self.bytes[X87_TAG_WORD_OFFSET]
    }

    /// MXCSR saved at offset 24.
    pub fn mxcsr(&self) -> u32 {
        u32::from_le_bytes([
            self.bytes[MXCSR_OFFSET],
            self.bytes[MXCSR_OFFSET + 1],
            self.bytes[MXCSR_OFFSET + 2],
            self.bytes[MXCSR_OFFSET + 3],
        ])
    }

    /// Copy another image into this one. Used to seed every task from the
    /// boot-time template.
    pub fn copy_from(&mut self, source: &Self) {
        // SAFETY: Both images are exactly FXSAVE_AREA_SIZE non-overlapping bytes.
        unsafe {
            core::ptr::copy_nonoverlapping(source.bytes.as_ptr(), self.bytes.as_mut_ptr(), FXSAVE_AREA_SIZE);
        }
    }

    /// Validate the image produced right after [`reset_state`] on this CPU.
    pub fn validate_initial_image(&self) -> bool {
        self.x87_control_word() == X87_CONTROL_WORD_DEFAULT
            && self.x87_tag_word() == X87_TAG_WORD_EMPTY
            && self.mxcsr() == MXCSR_DEFAULT
    }
}

/// Address of the FXSAVE image for the task currently executing.
///
/// Written by the scheduler and read by the IRQ assembly prologue. Zero means
/// the FPU path is not active yet and no save must happen.
#[unsafe(no_mangle)]
pub static SYSTEMES_CURRENT_FPU_AREA: AtomicUsize = AtomicUsize::new(0);

/// Address of the FXSAVE image to load before returning from an interrupt.
///
/// The IRQ assembly epilogue consumes and clears this slot so unrelated hardware
/// interrupts never reload a stale image.
#[unsafe(no_mangle)]
pub static SYSTEMES_NEXT_FPU_AREA: AtomicUsize = AtomicUsize::new(0);

unsafe fn read_cr0() -> u64 {
    let value: u64;
    // SAFETY: CR0 is readable at CPL0.
    unsafe { asm!("mov {0}, cr0", out(reg) value, options(nomem, nostack, preserves_flags)); }
    value
}

unsafe fn write_cr0(value: u64) {
    // SAFETY: Writing CR0 with EM/TS cleared and MP/NE set is valid at CPL0.
    unsafe { asm!("mov cr0, {0}", in(reg) value, options(nostack, preserves_flags)); }
}

unsafe fn read_cr4() -> u64 {
    let value: u64;
    // SAFETY: CR4 is readable at CPL0.
    unsafe { asm!("mov {0}, cr4", out(reg) value, options(nomem, nostack, preserves_flags)); }
    value
}

unsafe fn write_cr4(value: u64) {
    // SAFETY: Writing CR4 with OSFXSR/OSXMMEXCPT set is valid at CPL0.
    unsafe { asm!("mov cr4, {0}", in(reg) value, options(nostack, preserves_flags)); }
}

/// Enable x87/SSE state ownership and clear the lazy-switching traps.
///
/// - `CR0.EM = 0` lets the CPU execute x87 instructions instead of faulting.
/// - `CR0.MP = 1` pairs with `CR0.TS = 0` so no `#NM` is raised for FPU use.
/// - `CR0.NE = 1` reports FPU errors through `#MF` instead of legacy `FERR`.
/// - `CR4.OSFXSR = 1` enables `FXSAVE`/`FXRSTOR` and SSE.
/// - `CR4.OSXMMEXCPT = 1` reports SIMD exceptions through `#XM`.
///
/// Must run once during single-core boot with maskable interrupts disabled,
/// before any task uses FPU/SIMD state.
pub fn initialize() -> Result<(), FpuError> {
    // SAFETY: Boot-time, ring 0, interrupts disabled. The read-back below fails
    // closed if any bit does not latch.
    unsafe {
        let cr0 = read_cr0();
        let wanted_cr0 = (cr0 | REQUIRED_CR0_BITS) & !FORBIDDEN_CR0_BITS;
        if wanted_cr0 != cr0 {
            write_cr0(wanted_cr0);
        }
        if read_cr0() & (REQUIRED_CR0_BITS | FORBIDDEN_CR0_BITS) != REQUIRED_CR0_BITS {
            return Err(FpuError::ControlRegisterReadBackFailed);
        }

        let cr4 = read_cr4();
        let wanted_cr4 = cr4 | REQUIRED_CR4_BITS;
        if wanted_cr4 != cr4 {
            write_cr4(wanted_cr4);
        }
        if read_cr4() & REQUIRED_CR4_BITS != REQUIRED_CR4_BITS {
            return Err(FpuError::ControlRegisterReadBackFailed);
        }
    }
    reset_state();
    INITIALIZED.store(true, Ordering::Release);
    Ok(())
}

/// True once [`initialize`] succeeded on this CPU.
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}

/// Reset x87 and MXCSR to their architectural default values.
pub fn reset_state() {
    // SAFETY: Callers must have enabled OSFXSR/OSXMMEXCPT; no memory is touched
    // except the static MXCSR image.
    unsafe {
        asm!("fninit", options(nostack, preserves_flags));
        asm!("ldmxcsr [{0}]", in(reg) core::ptr::addr_of!(DEFAULT_MXCSR_VALUE), options(nostack, preserves_flags));
    }
}

/// Capture the live x87/SSE state into `area` with `FXSAVE64`.
///
/// # Safety
/// `area` must be 16-byte aligned and the FPU/SSE enable bits must be set.
pub unsafe fn capture(area: &mut FpuArea) {
    // SAFETY: The area is 16-byte aligned by its type and sized for FXSAVE64.
    unsafe { asm!("fxsave64 [{0}]", in(reg) area.address_mut(), options(nostack, preserves_flags)); }
}

/// Load x87/SSE state from `area` with `FXRSTOR64`.
///
/// # Safety
/// `area` must hold a complete image written by `FXSAVE64`.
pub unsafe fn restore(area: &FpuArea) {
    // SAFETY: The area is a complete, aligned FXSAVE64 image.
    unsafe { asm!("fxrstor64 [{0}]", in(reg) area.address(), options(nostack, preserves_flags)); }
}

/// Prove on the running CPU that `FXSAVE64`/`FXRSTOR64` preserve XMM and x87
/// state. Runs during boot with interrupts disabled.
pub fn verify_round_trip() -> bool {
    const PATTERN: [u8; 16] = [0xA5; 16];
    const ONE_BITS: u64 = 0x3FF0_0000_0000_0000; // 1.0_f64

    let mut area = FpuArea::zeroed();
    let mut observed_xmm = [0u8; 16];
    let mut observed_x87 = 0u64;

    // SAFETY: Boot-time, ring 0, FPU/SSE enabled by initialize().
    unsafe {
        asm!("movdqu xmm0, [{0}]", in(reg) PATTERN.as_ptr(), options(nostack, preserves_flags));
        asm!("fld1", options(nostack, preserves_flags));
        capture(&mut area);

        // Destroy the live state, then reload the captured image.
        asm!("pxor xmm0, xmm0", options(nostack, preserves_flags));
        asm!("fninit", options(nostack, preserves_flags));
        restore(&area);

        asm!("movdqu [{0}], xmm0", in(reg) observed_xmm.as_mut_ptr(), options(nostack, preserves_flags));
        asm!("fstp qword ptr [{0}]", in(reg) core::ptr::addr_of_mut!(observed_x87), options(nostack, preserves_flags));
    }

    let valid = observed_xmm == PATTERN
        && observed_x87 == ONE_BITS
        && area.x87_control_word() == X87_CONTROL_WORD_DEFAULT
        && area.mxcsr() == MXCSR_DEFAULT;
    // Leave a clean state for the scheduler template.
    reset_state();
    valid
}

/// Make `area` the image saved and restored for the running task.
pub fn set_current(area: &FpuArea) {
    SYSTEMES_CURRENT_FPU_AREA.store(area.address(), Ordering::Release);
}

/// Request that the IRQ epilogue reloads `area` once.
pub fn set_next(area: &FpuArea) {
    SYSTEMES_NEXT_FPU_AREA.store(area.address(), Ordering::Release);
}

/// Number of tasks currently expected by the FPU path (0 before initialization).
pub fn is_active() -> bool {
    SYSTEMES_CURRENT_FPU_AREA.load(Ordering::Acquire) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_layout_matches_fxsave64() {
        assert_eq!(core::mem::size_of::<FpuArea>(), FXSAVE_AREA_SIZE);
        assert_eq!(FXSAVE_AREA_SIZE, 512);
        assert_eq!(X87_CONTROL_WORD_OFFSET, 0);
        assert_eq!(X87_TAG_WORD_OFFSET, 4);
        assert_eq!(MXCSR_OFFSET, 24);
        assert_eq!(MXCSR_DEFAULT, 0x1F80);
        assert_eq!(X87_CONTROL_WORD_DEFAULT, 0x037F);
    }

    #[test]
    fn areas_are_16_byte_aligned() {
        let first = FpuArea::zeroed();
        let second = FpuArea::zeroed();
        assert_eq!(first.address() % 16, 0);
        assert_eq!(second.address() % 16, 0);
        assert_eq!(core::mem::align_of::<FpuArea>(), 16);
    }

    #[test]
    fn initial_image_validation_checks_documented_offsets() {
        let mut image = [0u8; FXSAVE_AREA_SIZE];
        image[0..2].copy_from_slice(&X87_CONTROL_WORD_DEFAULT.to_le_bytes());
        image[4] = X87_TAG_WORD_EMPTY;
        image[24..28].copy_from_slice(&MXCSR_DEFAULT.to_le_bytes());

        let mut area = FpuArea::zeroed();
        // SAFETY: The destination is a 512-byte image and the source is 512 bytes.
        unsafe { core::ptr::copy_nonoverlapping(image.as_ptr(), area.bytes.as_mut_ptr(), FXSAVE_AREA_SIZE); }
        assert!(area.validate_initial_image());

        area.bytes[24] = 0;
        assert!(!area.validate_initial_image());
    }

    #[test]
    fn copy_from_replaces_the_whole_image() {
        let mut source = FpuArea::zeroed();
        source.bytes[0] = 0x7F;
        source.bytes[4] = 0x00;
        let mut destination = FpuArea::zeroed();
        destination.copy_from(&source);
        assert_eq!(destination.as_bytes(), source.as_bytes());
    }

    #[test]
    fn current_and_next_slots_are_independent() {
        let mut first = FpuArea::zeroed();
        let second = FpuArea::zeroed();
        set_current(&mut first);
        set_next(&second);
        assert_eq!(SYSTEMES_CURRENT_FPU_AREA.load(Ordering::Acquire), first.address());
        assert_eq!(SYSTEMES_NEXT_FPU_AREA.load(Ordering::Acquire), second.address());
        SYSTEMES_CURRENT_FPU_AREA.store(0, Ordering::Release);
        SYSTEMES_NEXT_FPU_AREA.store(0, Ordering::Release);
    }
}
