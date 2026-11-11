//! xAPIC Local APIC timer initialization, PIT-referenced calibration and
//! INIT/SIPI validation for application processor bring-up.
//!
//! P1-6b: Per-CPU Local APIC timer initialization. Each CPU initializes its own
//! timer after setting up its GDT/IDT/TSS.

use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use crate::{acpi::MadtInfo, memory::PhysicalFrameAllocator, paging::IDENTITY_MAP_BYTES};

const IA32_APIC_BASE_MSR: u32 = 0x1B;
const APIC_BASE_MASK: u64 = 0x000F_FFFF_FFFF_F000;
const APIC_ENABLE: u64 = 1 << 11;
const APIC_X2_ENABLE: u64 = 1 << 10;
const CPUID_APIC: u32 = 1 << 9;
const APIC_SPURIOUS_VECTOR: u32 = 0xFF;
const TIMER_VECTOR: u8 = 0x40;
const LAPIC_TPR: usize = 0x80;
const LAPIC_EOI: usize = 0xB0;
const LAPIC_SVR: usize = 0xF0;
const LAPIC_ID: usize = 0x20;
const LAPIC_ICR_LOW: usize = 0x300;
const LAPIC_ICR_HIGH: usize = 0x310;
const LAPIC_ICR_DELIVERY_STATUS: u32 = 1 << 12;
const LAPIC_ICR_INIT_ASSERT: u32 = 0x0000_4500;
const LAPIC_ICR_STARTUP_MODE: u32 = 0x0000_0600;
const IPI_DELIVERY_TIMEOUT_ITERATIONS: usize = 10_000_000;
const PIT_INIT_SETTLE_TICKS: u16 = 11_932;
const PIT_STARTUP_SETTLE_TICKS: u16 = 239;
const LAPIC_LVT_TIMER: usize = 0x320;
const LAPIC_LVT_THERMAL: usize = 0x330;
const LAPIC_LVT_PERF: usize = 0x340;
const LAPIC_LVT_LINT0: usize = 0x350;
const LAPIC_LVT_LINT1: usize = 0x360;
const LAPIC_LVT_ERROR: usize = 0x370;
const LAPIC_TIMER_DIVIDE: usize = 0x3E0;
const LAPIC_TIMER_INITIAL: usize = 0x380;
const LAPIC_TIMER_CURRENT: usize = 0x390;
const PIT_CHANNEL2: u16 = 0x42;
const PIT_COMMAND: u16 = 0x43;
const PIT_SPEAKER: u16 = 0x61;
const PIT_INPUT_HZ: u32 = 1_193_182;
const PIT_CALIBRATION_COUNT: u16 = 59_659;
const PIT_CALIBRATION_TIMEOUT_ITERATIONS: usize = 100_000_000;
static APIC_ACTIVE: AtomicBool = AtomicBool::new(false);
static LOCAL_APIC_BASE: AtomicUsize = AtomicUsize::new(0);
static LOCAL_APIC_PHYSICAL: AtomicU64 = AtomicU64::new(0);
static BSP_APIC_ID: AtomicUsize = AtomicUsize::new(usize::MAX);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApicInitError {
    CpuApicUnsupported,
    X2ApicUnsupported,
    InvalidPhysicalAddress,
    MmioMappingFailed,
    TimerCalibrationInvalid,
    TimerCalibrationTimeout,
    TimerUnavailable,
    IpiUnavailable,
    IpiVectorInvalid,
    IpiTimingFailed,
    IpiTargetInvalid,
    IpiDeliveryPending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerCalibration {
    pub timer_hz: u64,
    pub target_hz: u32,
    pub initial_count: u32,
    pub pit_count: u16,
    pub elapsed_timer_counts: u32,
}

fn derive_timer_calibration(elapsed: u32, pit_count: u16, target_hz: u32) -> Option<TimerCalibration> {
    if elapsed == 0 || pit_count == 0 || target_hz == 0 { return None; }
    let timer_hz = (elapsed as u64).checked_mul(PIT_INPUT_HZ as u64)? / pit_count as u64;
    let initial_count = timer_hz / target_hz as u64;
    if timer_hz == 0 || initial_count == 0 || initial_count > u32::MAX as u64 { return None; }
    Some(TimerCalibration { timer_hz, target_hz, initial_count: initial_count as u32, pit_count, elapsed_timer_counts: elapsed })
}

unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: IA32_APIC_BASE is readable at CPL0 on CPUs exposing APIC.
    unsafe { asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack)); }
    ((high as u64) << 32) | low as u64
}

unsafe fn write_msr(msr: u32, value: u64) {
    // SAFETY: Caller validated the APIC MSR value and executes at CPL0.
    unsafe { asm!("wrmsr", in("ecx") msr, in("eax") value as u32, in("edx") (value >> 32) as u32, options(nomem, nostack)); }
}

unsafe fn lapic_write(base: *mut u8, offset: usize, value: u32) {
    // SAFETY: base is the UC-mapped Local APIC page and offset is a 32-bit register.
    unsafe { core::ptr::write_volatile(base.add(offset) as *mut u32, value); }
}

unsafe fn lapic_read(base: *mut u8, offset: usize) -> u32 {
    // SAFETY: base is the UC-mapped Local APIC page and offset is a 32-bit register.
    unsafe { core::ptr::read_volatile(base.add(offset) as *const u32) }
}

unsafe fn read_io_port(port: u16) -> u8 {
    let value: u8;
    // SAFETY: PIT channel-2 calibration uses the legacy 0x61 status/control port.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags)); }
    value
}

unsafe fn write_io_port(port: u16, value: u8) {
    // SAFETY: PIT channel-2 calibration writes the documented 0x42/0x43/0x61 ports.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)); }
}

fn mask_legacy_pic() {
    // SAFETY: The 8259 data ports are the legacy PIC mask registers.
    unsafe {
        asm!("out dx, al", in("dx") 0x21u16, in("al") 0xFFu8, options(nomem, nostack));
        asm!("out dx, al", in("dx") 0xA1u16, in("al") 0xFFu8, options(nomem, nostack));
    }
}

/// Enable xAPIC using the MADT physical base and map its registers UC.
pub unsafe fn initialize_local_apic(
    madt: &MadtInfo,
    allocator: &mut PhysicalFrameAllocator,
    page_table_root: u64,
) -> Result<(), ApicInitError> {
    // CPUID leaf 1 is architecturally available on x86_64.
    if core::arch::x86_64::__cpuid(1).edx & CPUID_APIC == 0 {
        return Err(ApicInitError::CpuApicUnsupported);
    }
    let physical = madt.local_apic_address;
    if physical == 0 || physical & 0xFFF != 0 || physical >= IDENTITY_MAP_BYTES {
        return Err(ApicInitError::InvalidPhysicalAddress);
    }
    // SAFETY: CPUID confirmed APIC support, so IA32_APIC_BASE is available.
    let current = unsafe { read_msr(IA32_APIC_BASE_MSR) };
    if current & APIC_X2_ENABLE != 0 { return Err(ApicInitError::X2ApicUnsupported); }
    let updated = (current & !(APIC_BASE_MASK | APIC_X2_ENABLE))
        | (physical & APIC_BASE_MASK) | APIC_ENABLE;
    // SAFETY: Physical base was checked for alignment and identity-map reachability.
    unsafe { write_msr(IA32_APIC_BASE_MSR, updated); }
    // SAFETY: The active identity map and allocator are valid; PAT UC is verified by the mapper.
    let lapic = unsafe { crate::paging::map_mmio_uncached(allocator, page_table_root, physical, 4096) }
        .map_err(|_| ApicInitError::MmioMappingFailed)?;
    unsafe {
        lapic_write(lapic, LAPIC_TPR, 0);
        for offset in [LAPIC_LVT_TIMER, LAPIC_LVT_THERMAL, LAPIC_LVT_PERF,
            LAPIC_LVT_LINT0, LAPIC_LVT_LINT1, LAPIC_LVT_ERROR] {
            lapic_write(lapic, offset, 1 << 16);
        }
        lapic_write(lapic, LAPIC_SVR, (1 << 8) | APIC_SPURIOUS_VECTOR);
        lapic_write(lapic, LAPIC_EOI, 0);
    }
    mask_legacy_pic();
    LOCAL_APIC_BASE.store(lapic as usize, Ordering::Release);
    LOCAL_APIC_PHYSICAL.store(physical, Ordering::Release);
    APIC_ACTIVE.store(true, Ordering::Release);
    BSP_APIC_ID.store(current_apic_id().unwrap_or(0) as usize, Ordering::Release);
    Ok(())
}

/// Calibrate the Local APIC countdown against PIT channel 2 and start a periodic timer.
pub unsafe fn calibrate_and_initialize_local_timer(target_hz: u32) -> Result<TimerCalibration, ApicInitError> {
    if !APIC_ACTIVE.load(Ordering::Acquire) || target_hz == 0 {
        return Err(ApicInitError::TimerUnavailable);
    }
    let base = LOCAL_APIC_BASE.load(Ordering::Acquire) as *mut u8;
    if base.is_null() { return Err(ApicInitError::TimerUnavailable); }
    let original_speaker = unsafe { read_io_port(PIT_SPEAKER) };
    unsafe { write_io_port(PIT_SPEAKER, original_speaker & !0x03); }
    unsafe { write_io_port(PIT_COMMAND, 0xB0); }
    unsafe { write_io_port(PIT_CHANNEL2, PIT_CALIBRATION_COUNT as u8); }
    unsafe { write_io_port(PIT_CHANNEL2, (PIT_CALIBRATION_COUNT >> 8) as u8); }
    unsafe {
        lapic_write(base, LAPIC_TIMER_DIVIDE, 0x3);
        lapic_write(base, LAPIC_LVT_TIMER, (1 << 16) | TIMER_VECTOR as u32);
        lapic_write(base, LAPIC_TIMER_INITIAL, u32::MAX);
    }
    if unsafe { read_io_port(PIT_SPEAKER) } & (1 << 5) != 0 {
        unsafe {
            lapic_write(base, LAPIC_LVT_TIMER, (1 << 16) | TIMER_VECTOR as u32);
            lapic_write(base, LAPIC_TIMER_INITIAL, 0);
            write_io_port(PIT_SPEAKER, original_speaker);
        }
        return Err(ApicInitError::TimerCalibrationInvalid);
    }
    unsafe { write_io_port(PIT_SPEAKER, (original_speaker & !0x02) | 0x01); }
    let mut completed = false;
    for _ in 0..PIT_CALIBRATION_TIMEOUT_ITERATIONS {
        if unsafe { read_io_port(PIT_SPEAKER) } & (1 << 5) != 0 {
            completed = true;
            break;
        }
        core::hint::spin_loop();
    }
    let current = unsafe { lapic_read(base, LAPIC_TIMER_CURRENT) };
    unsafe {
        lapic_write(base, LAPIC_LVT_TIMER, (1 << 16) | TIMER_VECTOR as u32);
        lapic_write(base, LAPIC_TIMER_INITIAL, 0);
        write_io_port(PIT_SPEAKER, original_speaker);
    }
    if !completed { return Err(ApicInitError::TimerCalibrationTimeout); }
    let elapsed = u32::MAX.saturating_sub(current);
    let calibration = derive_timer_calibration(elapsed, PIT_CALIBRATION_COUNT, target_hz)
        .ok_or(ApicInitError::TimerCalibrationInvalid)?;
    unsafe {
        lapic_write(base, LAPIC_TIMER_DIVIDE, 0x3);
        lapic_write(base, LAPIC_LVT_TIMER, TIMER_VECTOR as u32 | (1 << 17));
        lapic_write(base, LAPIC_TIMER_INITIAL, calibration.initial_count);
    }
    crate::clock::initialize();
    // Store the initial count for APs to use.
    set_timer_initial_count(calibration.initial_count);
    Ok(calibration)
}

/// Initialize the Local APIC timer on this CPU using a pre-calibrated initial count.
/// This is used by application processors after the BSP has calibrated the timer.
///
/// # Safety
///
/// Must be called after the Local APIC is initialized on this CPU (via
/// `initialize_local_apic`). The caller must ensure the timer is not already running.
pub unsafe fn initialize_local_timer_on_cpu(initial_count: u32) -> Result<(), ApicInitError> {
    if !APIC_ACTIVE.load(Ordering::Acquire) || initial_count == 0 {
        return Err(ApicInitError::TimerUnavailable);
    }
    // IA32_APIC_BASE is per logical processor. Explicitly enable this AP's
    // local controller at the ACPI-discovered physical address after INIT/SIPI.
    let physical = LOCAL_APIC_PHYSICAL.load(Ordering::Acquire);
    if physical == 0 { return Err(ApicInitError::TimerUnavailable); }
    let current = unsafe { read_msr(IA32_APIC_BASE_MSR) };
    if current & APIC_X2_ENABLE != 0 { return Err(ApicInitError::X2ApicUnsupported); }
    let updated = (current & !(APIC_BASE_MASK | APIC_X2_ENABLE))
        | (physical & APIC_BASE_MASK) | APIC_ENABLE;
    unsafe { write_msr(IA32_APIC_BASE_MSR, updated); }
    let base = LOCAL_APIC_BASE.load(Ordering::Acquire) as *mut u8;
    if base.is_null() {
        return Err(ApicInitError::TimerUnavailable);
    }
    unsafe {
        // APs receive INIT/SIPI with their own Local APIC software-enable state.
        // Enable the local controller and clear its task priority before unmasking
        // the periodic timer; BSP initialization does not set the AP's SVR.
        lapic_write(base, LAPIC_TPR, 0);
        lapic_write(base, LAPIC_SVR, (1 << 8) | APIC_SPURIOUS_VECTOR);
        lapic_write(base, LAPIC_EOI, 0);
        lapic_write(base, LAPIC_TIMER_DIVIDE, 0x3);
        lapic_write(base, LAPIC_LVT_TIMER, TIMER_VECTOR as u32 | (1 << 17)); // periodic mode
        lapic_write(base, LAPIC_TIMER_INITIAL, initial_count);
    }
    Ok(())
}

/// Get the calibrated initial count for the timer (set by BSP during calibration).
static TIMER_INITIAL_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Store the calibrated initial count for use by APs.
pub fn set_timer_initial_count(count: u32) {
    TIMER_INITIAL_COUNT.store(count as usize, Ordering::Release);
}

/// Get the calibrated initial count.
pub fn get_timer_initial_count() -> u32 {
    TIMER_INITIAL_COUNT.load(Ordering::Acquire) as u32
}

pub fn is_active() -> bool { APIC_ACTIVE.load(Ordering::Acquire) }

/// Read the executing CPU's Local APIC ID, or `None` while the LAPIC
/// is inactive.
/// True only on the bootstrap processor whose Local APIC ID was captured at init.
pub fn is_bootstrap_cpu() -> bool {
    current_apic_id().map(|id| id as usize == BSP_APIC_ID.load(Ordering::Acquire)).unwrap_or(false)
}

pub fn current_apic_id() -> Option<u8> {
    if !APIC_ACTIVE.load(Ordering::Acquire) {
        return None;
    }
    let base = LOCAL_APIC_BASE.load(Ordering::Acquire) as *mut u8;
    if base.is_null() {
        return None;
    }
    // SAFETY: The UC-mapped Local APIC page was published after a successful
    // initialization; the ID register is read-only MMIO.
    let id_register = unsafe { lapic_read(base, LAPIC_ID) };
    Some(((id_register >> 24) & 0xFF) as u8)
}

/// Wait for the ICR delivery-status bit to clear. QEMU clears it as soon as
/// the interrupt is queued; real chipsets raise it while the command trips
/// through the bus, so the bounded spin covers both without any timer.
unsafe fn wait_for_icr_delivery(base: *mut u8) -> Result<(), ApicInitError> {
    for _ in 0..IPI_DELIVERY_TIMEOUT_ITERATIONS {
        // SAFETY: The UC-mapped Local APIC page is active and ICR_LOW is a
        // 32-bit register.
        if unsafe { lapic_read(base, LAPIC_ICR_LOW) } & LAPIC_ICR_DELIVERY_STATUS == 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(ApicInitError::IpiDeliveryPending)
}

/// Hold the caller for `ticks` PIT input clocks by letting channel 2 drain
/// one-shot, mirroring the calibration path's programming sequence. Returns
/// false when the OUT bit never rises inside the calibration budget.
unsafe fn pit_channel2_delay_ticks(ticks: u16) -> bool {
    let original_speaker = unsafe { read_io_port(PIT_SPEAKER) };
    unsafe { write_io_port(PIT_SPEAKER, original_speaker & !0x03); }
    unsafe { write_io_port(PIT_COMMAND, 0xB0); }
    unsafe { write_io_port(PIT_CHANNEL2, ticks as u8); }
    unsafe { write_io_port(PIT_CHANNEL2, (ticks >> 8) as u8); }
    unsafe { write_io_port(PIT_SPEAKER, (original_speaker & !0x02) | 0x01); }
    let mut settled = false;
    for _ in 0..PIT_CALIBRATION_TIMEOUT_ITERATIONS {
        if unsafe { read_io_port(PIT_SPEAKER) } & (1 << 5) != 0 {
            settled = true;
            break;
        }
        core::hint::spin_loop();
    }
    unsafe { write_io_port(PIT_SPEAKER, original_speaker); }
    settled
}

/// Deliver the INIT / SIPI / SIPI handshake to the application processor with
/// `apic_id` so it fetches its first trampoline instruction from the 4 KiB
/// page at `vector_page` (a page-aligned address below 1 MiB whose vector
/// byte is nonzero). The caller must hold interrupts disabled; delivery
/// waits are bounded spins plus PIT one-shots, never timer interrupts.
pub unsafe fn send_init_and_startup_ipi(apic_id: u8, vector_page: u64) -> Result<(), ApicInitError> {
    let vector = vector_page >> 12;
    if vector_page & (crate::memory::PAGE_SIZE - 1) != 0 || vector == 0 || vector > 0xFF {
        return Err(ApicInitError::IpiVectorInvalid);
    }
    if !APIC_ACTIVE.load(Ordering::Acquire) {
        return Err(ApicInitError::IpiUnavailable);
    }
    let base = LOCAL_APIC_BASE.load(Ordering::Acquire) as *mut u8;
    if base.is_null() {
        return Err(ApicInitError::IpiUnavailable);
    }
    let destination = (apic_id as u32) << 24;
    // INIT assertion, then the ~10 ms reset window the MP protocol requires
    // before the first STARTUP interrupt.
    // SAFETY: The UC-mapped Local APIC page is active; both ICR halves are
    // separate 32-bit registers and the destination is written first.
    unsafe {
        lapic_write(base, LAPIC_ICR_HIGH, destination);
        lapic_write(base, LAPIC_ICR_LOW, LAPIC_ICR_INIT_ASSERT);
    }
    unsafe { wait_for_icr_delivery(base)?; }
    if !unsafe { pit_channel2_delay_ticks(PIT_INIT_SETTLE_TICKS) } {
        return Err(ApicInitError::IpiTimingFailed);
    }
    for _ in 0..2 {
        // SAFETY: Same published Local APIC mapping as the INIT write above.
        unsafe {
            lapic_write(base, LAPIC_ICR_HIGH, destination);
            lapic_write(base, LAPIC_ICR_LOW, LAPIC_ICR_STARTUP_MODE | vector as u32);
        }
        unsafe { wait_for_icr_delivery(base)?; }
        // A second start-up interrupt follows ~200 µs later for chipsets
        // that drop the first delivery; CPUs already running ignore it.
        if !unsafe { pit_channel2_delay_ticks(PIT_STARTUP_SETTLE_TICKS) } {
            return Err(ApicInitError::IpiTimingFailed);
        }
    }
    Ok(())
}

pub unsafe fn stop_local_timer() {
    let base = LOCAL_APIC_BASE.load(Ordering::Acquire) as *mut u8;
    if !base.is_null() {
        // SAFETY: The Local APIC base was published after a successful UC mapping.
        unsafe { lapic_write(base, LAPIC_LVT_TIMER, (1 << 16) | TIMER_VECTOR as u32); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn systemes_apic_timer_dispatch(vector: u64, saved_rsp: usize) -> usize {
    match vector {
        0x40 => {
            if is_bootstrap_cpu() {
                crate::clock::on_timer_interrupt();
            }
            let base = LOCAL_APIC_BASE.load(Ordering::Acquire) as *mut u8;
            if !base.is_null() {
                // SAFETY: Timer IRQ executes only after the UC-mapped LAPIC is enabled.
                unsafe { lapic_write(base, LAPIC_EOI, 0); }
            }
            crate::scheduler::on_timer(saved_rsp)
        }
        0x41 => crate::scheduler::on_yield(saved_rsp),
        _ => saved_rsp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_uses_measured_counter_delta() {
        let calibration = derive_timer_calibration(8_000_000, PIT_CALIBRATION_COUNT, 100).unwrap();
        assert_eq!(calibration.target_hz, 100);
        assert!(calibration.timer_hz > 0);
        assert!(calibration.initial_count > 0);
    }

    #[test]
    fn calibration_rejects_zero_or_unrepresentable_measurements() {
        assert_eq!(derive_timer_calibration(0, PIT_CALIBRATION_COUNT, 100), None);
        assert_eq!(derive_timer_calibration(1, 0, 100), None);
        assert_eq!(derive_timer_calibration(1, PIT_CALIBRATION_COUNT, 0), None);
        assert_eq!(derive_timer_calibration(u32::MAX, 1, 1), None);
    }
}
