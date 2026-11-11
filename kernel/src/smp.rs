//! Multicore foundation: application processor bring-up over INIT/SIPI.
//!
//! This module implements the first SMP milestone (P1-6a):
//!
//! 1. Enumerate enabled processors from the ACPI MADT, skipping the
//!    bootstrap CPU identified through its resident Local APIC.
//! 2. Publish a self-contained real-mode/32-bit/long-mode bring-up trampoline
//!    in a reserved page below 1 MiB — the only region a STARTUP IPI vector
//!    can address.
//! 3. Wake every enabled application processor one at a time through the
//!    Local APIC ICR INIT/SIPI sequence.
//! 4. Each AP runs the real-mode stage, enters 32-bit protected mode with a
//!    page-based code descriptor, enables PAE/LME/NXE on the bootstrap CR3,
//!    reads its own Local APIC ID from MMIO through flat selectors, and enters
//!    kernel long mode through a far jump to the patched `systemes_ap_entry`
//!    address. Every low-memory control transfer is either page-relative or
//!    explicitly patched, never dependent on a segment base long mode ignores.
//! 5. The SIPI page is reserved before the identity map allocates its tables,
//!    because those tables are taken from the lowest physical frames.
//! 6. Bootstrap CPU and APs execute the same validation: private counters,
//!    a relaxed shared counter and increments guarded by the kernel's own
//!    `SpinLock` — which only stabilizes when cross-core memory coherence
//!    and the lock itself behave correctly on real symmetric harts.
//!
//! P1-6c scheduler integration:
//! - Each AP installs its private GDT/IDT/TSS and calibrated Local APIC timer,
//!   initializes a CPU-local scheduler, then enters the scheduler idle loop.
//! - Runnable task descriptors may be transferred between per-CPU task tables;
//!   stack and FXSAVE resources remain pinned while both run-queue locks are held.
//! - The BSP-only SYSCALL/SYSRET and Ring 3 assembly stack ABI remains a separate
//!   limitation; per-CPU syscall entry is not implemented by this scheduler phase.
//! - The trampoline page (< 1 MiB) stays reserved for AP bring-up.
//! - The kernel CR3 must sit below 4 GiB because the 32-bit trampoline stage
//!   loads it with a 32-bit immediate. QEMU/OVMF images satisfy this today.

use crate::acpi::MadtInfo;
use crate::memory::{PhysicalFrameAllocator, PAGE_SIZE};
use crate::paging::MemoryRegion;
use crate::sync::SpinLock;
use crate::interrupts::{initialize_cpu, MAX_CPUS};
use crate::apic::{initialize_local_timer_on_cpu, get_timer_initial_count};
use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicU64, Ordering};

/// Number of Local APIC IDs the per-AP slot arrays index (MADT IDs here).
pub const MAX_CPUS_PER_APIC_ID: usize = 64;
/// Largest payload that still leaves scratch space inside the 4 KiB page.
pub const TRAMPOLINE_MAX_BYTES: usize = 0x0F00;
/// Stack bytes allocated for every started application processor.
const AP_STACK_BYTES: usize = 8 * 1024;
/// Rounds every CPU runs while exercising the shared counters and lock.
const VALIDATION_ROUNDS: u64 = 250;
/// Low-memory page claimed by `reserve_trampoline_page` before the identity
/// map allocates its tables from the same region.
static RESERVED_TRAMPOLINE_PAGE: AtomicU64 = AtomicU64::new(0);
/// Upper bound of spin iterations to wait for one AP to reach long mode.
const AP_START_TIMEOUT_ITERATIONS: usize = 400_000_000;
/// Upper bound of spin iterations to wait for one AP to park.
const AP_PARK_TIMEOUT_ITERATIONS: usize = 400_000_000;

const SLOT_STATE_STARTED: u64 = 1;
const SLOT_STATE_PARKED: u64 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmpBringUpError {
    ApicUnavailable,
    NoApplicationProcessors,
    NoLowMemoryPage,
    TrampolineTooLarge,
    Crash3Beyond4GiB,
    KernelEntryBeyond4GiB,
    LapicBaseBeyond4GiB,
    ApicIdOutOfRange,
    MissingPatchSite,
    AmbiguousPatchSite,
    TrampolineRxVerificationFailed,
    ApStackAllocationFailed,
    ApDidNotStart { apic_id: u8 },
    ApDidNotPark { apic_id: u8 },
    CrossCoreCounterMismatch,
}

/// Outcome of a successful bring-up; the boot path prints these numbers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmpSummary {
    /// Enabled MADT processors other than the bootstrap CPU.
    pub discovered_count: usize,
    /// APs that reached kernel long mode on the shared CR3.
    pub started_apic_ids: [u8; MAX_CPUS_PER_APIC_ID],
    pub started_count: usize,
    /// Local APIC ID of the bootstrap CPU.
    pub bsp_apic_id: u8,
    /// Physical address of the low-memory trampoline page.
    pub trampoline_page: u64,
    /// Relaxed shared counter total after every CPU joined validation.
    pub shared_relaxed_total: u64,
    /// SpinLock-guarded counter total after every CPU joined validation.
    pub spin_locked_total: u64,
}

/// Per-AP stack tops by Local APIC ID; the naked AP entry loads one before
/// any Rust code runs on the AP.
#[unsafe(no_mangle)]
static mut SYSTEMES_AP_STACKS: [u64; MAX_CPUS_PER_APIC_ID] = [0; MAX_CPUS_PER_APIC_ID];

static AP_STATES: [AtomicU64; MAX_CPUS_PER_APIC_ID] =
    [const { AtomicU64::new(0) }; MAX_CPUS_PER_APIC_ID];
static AP_PRIVATE_COUNTERS: [AtomicU64; MAX_CPUS_PER_APIC_ID] =
    [const { AtomicU64::new(0) }; MAX_CPUS_PER_APIC_ID];

static SHARED_RELAXED_COUNTER: AtomicU64 = AtomicU64::new(0);
/// Cross-core counter whose every update is serialized by `SpinLock`.
static SHARED_LOCKED_COUNTER: SpinLock<u64> = SpinLock::new(0);

// Patch sites are located by unique little-endian 32-bit/64-bit markers so
// that runtime addresses never depend on fragile copy offsets.
const MAGIC_GDT_POINTER_BASE: u32 = 0x4C44_4701;
const MAGIC_CS16_DESCRIPTOR_BASE: u32 = 0x4C44_4702;
const MAGIC_DS16_DESCRIPTOR_BASE: u32 = 0x4C44_4703;
const MAGIC_TRAMPOLINE_CR3: u32 = 0x4C44_4704;
const MAGIC_TRAMPOLINE_LAPIC_BASE: u32 = 0x4C44_4740;
const MAGIC_TRAMPOLINE_AP_ENTRY: u32 = 0x4C44_4751;
/// Absolute address of the far pointer read by the 32-bit stage.
const MAGIC_TRAMPOLINE_ENTRY_ADDR: u32 = 0x4C44_4752;

unsafe extern "C" {
    static systemes_ap_trampoline: u8;
    static systemes_ap_trampoline_end: u8;
    /// Start of the trampoline's private GDT inside the payload.
    static ap_tramp_gdt: u8;
    /// Far pointer (offset then long-mode selector) consumed by the
    /// 32-bit stage once it switched to flat data selectors.
    static ap_tramp_kernel_entry: u8;
}

global_asm!(
    r#"
.section .text.systemes_ap_trampoline, "ax"
.global systemes_ap_trampoline
systemes_ap_trampoline:
.code16
    /* AP entry: CS = vector page segment, IP = 0, interrupts off. Mirror CS
       into DS so displacements below are page-relative physical offsets. */
    movw %cs, %ax
    movw %ax, %ds
    /* Load the trampoline's own GDT; its pointer's base field is patched
       with the physical page address so the descriptors live in the copy. */
    lgdtl (ap_tramp_gdt_ptr - systemes_ap_trampoline)
    /* Real mode -> protected mode. Paging stays off, so linear equals
       physical and the trampoline page range stays reachable. */
    movl %cr0, %eax
    orl $0x1, %eax
    movl %eax, %cr0
    /* Far reload CS with a 32-bit code descriptor whose base is the
       trampoline page: EIP stays a page-relative offset and the .code32
       stage below is assembled for that segment's default operand size. */
    ljmp $0x08, $(.Lap_tramp_stage2 - systemes_ap_trampoline)
.Lap_tramp_stage2:
.code32
    /* Reload all data selectors from the page-based data descriptor so the
       displacements below are already page-relative under paging. */
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %fs
    movw %ax, %gs
    movw %ax, %ss
    /* Enable PAE. */
    movl %cr4, %eax
    orl $0x20, %eax
    movl %eax, %cr4
    /* Assert EFER.LME and EFER.NXE ahead of paging: the kernel identity map
       marks data leaves NX, and without NXE those bits are reserved. */
    movl $0xC0000080, %ecx
    rdmsr
    orl $0x900, %eax
    wrmsr
    /* Replace CR3 with the bootstrap CPU's kernel identity map. */
    movl (ap_tramp_cr3 - systemes_ap_trampoline), %eax
    movl %eax, %cr3
    /* Enable paging; the trampoline page is RX there, so control flow
       continues in place, one translation behind the same addresses. */
    movl %cr0, %eax
    orl $0x80000000, %eax
    movl %eax, %cr0
    /* Fetch the LAPIC base while DS still carries the page base, then switch
       every data selector to the flat descriptor: 32-bit paging adds the
       segment base, and MMIO or kernel addresses must not be offset by it. */
    movl (ap_tramp_lapic_base - systemes_ap_trampoline), %ebx
    movw $0x18, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %fs
    movw %ax, %gs
    movw %ax, %ss
    /* Read this CPU's Local APIC ID from the xAPIC register page. */
    movl 0x20(%ebx), %eax
    shrl $24, %eax
    andl $0xFF, %eax
    movl %eax, %edi
    /* Far jump straight into the kernel's 64-bit entry point. The BSP
       patches the operand with the absolute linear address of the far
       pointer, which itself carries systemes_ap_entry and selector 0x20. */
    ljmp *({MAGIC_TRAMPOLINE_ENTRY_ADDR})

    ud2

ap_tramp_gdt_ptr:
    .word (ap_tramp_gdt_end - ap_tramp_gdt) - 1
    .long {MAGIC_GDT_POINTER_BASE}

ap_tramp_gdt:
    .quad 0
    /* 0x08: 16-bit protected mode code. The patched 4-byte slot covers
       base[0..24] and the attribute byte; the trailing bytes keep the
       descriptor 8 bytes long. */
    .byte 0xFF, 0xFF
    .long {MAGIC_CS16_DESCRIPTOR_BASE}
    .byte 0x4F, 0
    /* 0x10: 32-bit protected mode data with the same page base. */
    .byte 0xFF, 0xFF
    .long {MAGIC_DS16_DESCRIPTOR_BASE}
    .byte 0x4F, 0
    /* 0x18: flat 32-bit data, base 0, limit 4 GiB, read/write. */
    .quad 0x00CF92000000FFFF
    /* 0x20: 64-bit code, flat base, G=1, L=1. */
    .quad 0x00AF9A000000FFFF
ap_tramp_gdt_end:

ap_tramp_cr3:
    .long {MAGIC_TRAMPOLINE_CR3}
    .long 0

ap_tramp_lapic_base:
    .long {MAGIC_TRAMPOLINE_LAPIC_BASE}

ap_tramp_kernel_entry:
    .long {MAGIC_TRAMPOLINE_AP_ENTRY}
    .word 0x20

.global systemes_ap_trampoline_end
systemes_ap_trampoline_end:
    nop
"#,
    MAGIC_GDT_POINTER_BASE = const MAGIC_GDT_POINTER_BASE,
    MAGIC_CS16_DESCRIPTOR_BASE = const MAGIC_CS16_DESCRIPTOR_BASE,
    MAGIC_DS16_DESCRIPTOR_BASE = const MAGIC_DS16_DESCRIPTOR_BASE,
    MAGIC_TRAMPOLINE_CR3 = const MAGIC_TRAMPOLINE_CR3,
    MAGIC_TRAMPOLINE_LAPIC_BASE = const MAGIC_TRAMPOLINE_LAPIC_BASE,
    MAGIC_TRAMPOLINE_AP_ENTRY = const MAGIC_TRAMPOLINE_AP_ENTRY,
    MAGIC_TRAMPOLINE_ENTRY_ADDR = const MAGIC_TRAMPOLINE_ENTRY_ADDR,
    options(att_syntax),
);

/// Copy of the linked trampoline payload; includes the terminator symbol.
fn trampoline_bytes() -> &'static [u8] {
    // SAFETY: The linker places both symbols inside the kernel image and the
    // end marker immediately follows the payload within the same section.
    unsafe {
        let start = core::ptr::addr_of!(systemes_ap_trampoline);
        let end = core::ptr::addr_of!(systemes_ap_trampoline_end);
        let length = (end as usize).wrapping_sub(start as usize);
        core::slice::from_raw_parts(start, length)
    }
}

fn find_patch_site(payload: &[u8], magic: &[u8]) -> Result<usize, SmpBringUpError> {
    let mut found: Option<usize> = None;
    for index in 0..payload.len() {
        if payload.len() < index + magic.len() {
            break;
        }
        if &payload[index..index + magic.len()] == magic {
            if found.is_some() {
                return Err(SmpBringUpError::AmbiguousPatchSite);
            }
            found = Some(index);
        }
    }
    found.ok_or(SmpBringUpError::MissingPatchSite)
}

fn verify_unique_magics(payload: &[u8]) -> Result<(), SmpBringUpError> {
    for magic in [
        MAGIC_GDT_POINTER_BASE.to_le_bytes(),
        MAGIC_CS16_DESCRIPTOR_BASE.to_le_bytes(),
        MAGIC_DS16_DESCRIPTOR_BASE.to_le_bytes(),
        MAGIC_TRAMPOLINE_CR3.to_le_bytes(),
        MAGIC_TRAMPOLINE_LAPIC_BASE.to_le_bytes(),
        MAGIC_TRAMPOLINE_AP_ENTRY.to_le_bytes(),
        MAGIC_TRAMPOLINE_ENTRY_ADDR.to_le_bytes(),
    ] {
        let _ = find_patch_site(payload, &magic)?;
    }
    Ok(())
}


fn patch_u32(payload: &mut [u8], magic: u32, value: u32) -> Result<(), SmpBringUpError> {
    let site = find_patch_site(payload, magic.to_le_bytes().as_ref())?;
    payload[site..site + 4].copy_from_slice(value.to_le_bytes().as_slice());
    Ok(())
}

/// Patch a 16-bit descriptor whose base field covers 24 bits of the page
/// address; the fourth patched byte carries the attribute byte verbatim.
#[cfg(test)]
fn patch_u64(payload: &mut [u8], magic: u64, value: u64) -> Result<(), SmpBringUpError> {
    let site = find_patch_site(payload, magic.to_le_bytes().as_ref())?;
    payload[site..site + 8].copy_from_slice(value.to_le_bytes().as_slice());
    Ok(())
}

/// Patch a 16-bit descriptor whose base field covers 24 bits of the page
/// address; the fourth patched byte carries the attribute byte verbatim.
fn patch_descriptor_base(
    payload: &mut [u8],
    magic: u32,
    page: u64,
    attributes: u8,
) -> Result<(), SmpBringUpError> {
    if page >= 0x0100_0000 {
        return Err(SmpBringUpError::AmbiguousPatchSite);
    }
    let base32 = ((page & 0x00FF_FFFF) as u32) | ((attributes as u32) << 24);
    // The magic occupies descriptor bytes [base0..7, base8..15, base16..23,
    // attribute]: value byte 0..2 carry the page's low 24 bits and value
    // byte 3 keeps the descriptor attribute byte in place.
    patch_u32(payload, magic, base32)
}

/// The same validation both CPUs run once they reach the kernel.
fn run_core_validation(slot: usize) {
    for _ in 0..VALIDATION_ROUNDS {
        AP_PRIVATE_COUNTERS[slot].fetch_add(1, Ordering::Relaxed);
        SHARED_RELAXED_COUNTER.fetch_add(1, Ordering::Relaxed);
        {
            // SAFETY: The guard serializes exclusive access through the lock.
            let mut guard = SHARED_LOCKED_COUNTER.lock();
            let value = *guard;
            *guard = value.wrapping_add(1);
        }
    }
}

/// Bootstrap-side wait helper for monotone slot states; an AP can advance
/// past `expected` before the bootstrap CPU looks, so compare by rank.
fn wait_for_slot_state_at_least(slot: usize, expected: u64, iterations: usize) -> bool {
    for _ in 0..iterations {
        if AP_STATES[slot].load(Ordering::Acquire) >= expected {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// Bootstrap-side wait helper; returns false only past the iteration bound.
fn wait_for_slot_state(slot: usize, expected: u64, iterations: usize) -> bool {
    for _ in 0..iterations {
        if AP_STATES[slot].load(Ordering::Acquire) == expected {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// Wake every enabled application processor beyond the bootstrap CPU and
/// validate cross-core synchronization. Interrupts on the calling CPU are
/// held off for the duration and restored on exit.
///
/// Claim the physical page STARTUP interrupts will execute from.
///
/// Must run once on the bootstrap CPU, after the frame allocator exists and
/// before the identity map allocates its tables out of the first MiB. The
/// page stays allocated for the whole boot and is never handed out again.
pub fn reserve_trampoline_page(allocator: &mut PhysicalFrameAllocator) -> Option<u64> {
    // The SIPI vector is a page number in 0..=255, so the page must sit
    // strictly inside the first MiB and must not be the null page.
    let page = allocator.allocate_frame_below(0x0010_0000)?;
    if page < PAGE_SIZE || (page >> 12) & 0xFF == 0 {
        return None;
    }
    RESERVED_TRAMPOLINE_PAGE.store(page, Ordering::Release);
    Some(page)
}

/// # Safety
///
/// The kernel page-table root must already be active, the Local APIC must
/// be initialized and mapped, and the global allocator must be initialized.
/// Only the bootstrap CPU may call this, once, before APs are reused.
pub unsafe fn bring_up_application_processors(
    madt: &MadtInfo,
    allocator: &mut PhysicalFrameAllocator,
    page_table_root: u64,
) -> Result<SmpSummary, SmpBringUpError> {
    // Preserve the caller's interrupt flag and hold IRQs off for bring-up.
    let mut interrupt_flag: u64;
    // SAFETY: The flag save/restore pair brackets every privileged operation.
    unsafe {
        asm!("pushfq", "pop {}", "cli", out(reg) interrupt_flag, options(nomem, nostack));
    }

    let outcome = unsafe {
        bring_up_application_processors_inner(madt, allocator, page_table_root)
    };

    // SAFETY: Restore the caller's interrupt flag after bring-up completes.
    unsafe {
        if interrupt_flag & (1 << 9) != 0 {
            asm!("sti", options(nomem, nostack));
        }
    }
    outcome
}

unsafe fn bring_up_application_processors_inner(
    madt: &MadtInfo,
    allocator: &mut PhysicalFrameAllocator,
    page_table_root: u64,
) -> Result<SmpSummary, SmpBringUpError> {
    let bsp_apic_id = match crate::apic::current_apic_id() {
        Some(id) => id,
        None => return Err(SmpBringUpError::ApicUnavailable),
    };

    let mut candidate_ids: [u8; MAX_CPUS_PER_APIC_ID] = [0; MAX_CPUS_PER_APIC_ID];
    let mut candidate_count = 0usize;
    if bsp_apic_id as usize >= MAX_CPUS_PER_APIC_ID {
        return Err(SmpBringUpError::ApicIdOutOfRange);
    }
    for entry in madt.local_apics[..madt.local_apic_count].iter() {
        if entry.flags & 1 == 0 {
            continue;
        }
        if entry.apic_id == bsp_apic_id {
            continue;
        }
        if entry.apic_id as usize >= MAX_CPUS_PER_APIC_ID {
            return Err(SmpBringUpError::ApicIdOutOfRange);
        }
        candidate_ids[candidate_count] = entry.apic_id;
        candidate_count += 1;
    }
    if candidate_count == 0 {
        return Err(SmpBringUpError::NoApplicationProcessors);
    }

    // The SIPI vector addresses physical pages only below 1 MiB, and the
    // identity map consumes that region for its own tables at boot, so the
    // boot path must have reserved the page before paging was built.
    let trampoline_page = match RESERVED_TRAMPOLINE_PAGE.load(Ordering::Acquire) {
        page if page >= PAGE_SIZE && (page >> 12) & 0xFF != 0 => page,
        _ => return Err(SmpBringUpError::NoLowMemoryPage),
    };

    let source = trampoline_bytes();
    if source.len() > TRAMPOLINE_MAX_BYTES {
        return Err(SmpBringUpError::TrampolineTooLarge);
    }
    if verify_unique_magics(source).is_err() {
        return Err(SmpBringUpError::AmbiguousPatchSite);
    }
    // SAFETY: The page is allocator-managed RAM and mapped through the UEFI
    // identity map until AP execution begins; the copy happens before the RX
    // protection below switches it to read-execute.
    let mut payload: &mut [u8] = unsafe {
        core::slice::from_raw_parts_mut(trampoline_page as *mut u8, TRAMPOLINE_MAX_BYTES)
    };
    payload[..source.len()].copy_from_slice(source);

    if page_table_root > u32::MAX as u64 {
        return Err(SmpBringUpError::Crash3Beyond4GiB);
    }
    let kernel_entry = systemes_ap_entry as *const () as usize as u64;
    if kernel_entry > u32::MAX as u64 {
        return Err(SmpBringUpError::KernelEntryBeyond4GiB);
    }
    if madt.local_apic_address > u32::MAX as u64 {
        return Err(SmpBringUpError::LapicBaseBeyond4GiB);
    }
    // The GDTR base must address the GDT, not the payload page start.
    let gdt_offset = core::ptr::addr_of!(ap_tramp_gdt) as usize
        - core::ptr::addr_of!(systemes_ap_trampoline) as usize;
    let gdt_address = trampoline_page
        .checked_add(gdt_offset as u64)
        .ok_or(SmpBringUpError::MissingPatchSite)?;
    patch_u32(&mut payload, MAGIC_GDT_POINTER_BASE, gdt_address as u32)?;
    patch_descriptor_base(&mut payload, MAGIC_CS16_DESCRIPTOR_BASE, trampoline_page, 0x9A)?;
    patch_descriptor_base(&mut payload, MAGIC_DS16_DESCRIPTOR_BASE, trampoline_page, 0x92)?;
    patch_u32(&mut payload, MAGIC_TRAMPOLINE_CR3, page_table_root as u32)?;
    patch_u32(&mut payload, MAGIC_TRAMPOLINE_LAPIC_BASE, madt.local_apic_address as u32)?;
    patch_u32(
        &mut payload,
        MAGIC_TRAMPOLINE_AP_ENTRY,
        kernel_entry as u32,
    )?;
    // The 32-bit stage reaches the far pointer through flat selectors, so
    // its operand is an absolute linear address inside the trampoline page.
    let entry_offset = core::ptr::addr_of!(ap_tramp_kernel_entry) as usize
        - core::ptr::addr_of!(systemes_ap_trampoline) as usize;
    let entry_address = trampoline_page
        .checked_add(entry_offset as u64)
        .ok_or(SmpBringUpError::MissingPatchSite)?;
    patch_u32(
        &mut payload,
        MAGIC_TRAMPOLINE_ENTRY_ADDR,
        entry_address as u32,
    )?;

    // Publish the trampoline page as RX in the shared kernel page map.
    let rx_region = [MemoryRegion {
        start: trampoline_page,
        end: trampoline_page + PAGE_SIZE,
        permissions: crate::paging::PagePermissions::ReadOnlyExecute,
    }];
    // SAFETY: The page table root is active, the allocator is valid, and the
    // trampoline page is allocated from the managed physical memory.
    if unsafe {
        crate::paging::protect_regions(allocator, page_table_root, &rx_region)
            .and_then(|()| crate::paging::verify_regions(page_table_root, &rx_region))
    }
    .is_err()
    {
        return Err(SmpBringUpError::TrampolineRxVerificationFailed);
    }

    // Instruction caches do not snoop every cross-core code-store: flush all
    // dirty lines so each AP fetches the patched payload.
    // SAFETY: WBINVD is a serializing instruction available at CPL0.
    unsafe {
        asm!("mfence", "wbinvd", options(nostack));
    }

    // Allocate one kernel stack per AP and publish it by Local APIC ID.
    for index in 0..candidate_count {
        let apic_id = candidate_ids[index] as usize;
        let layout = match core::alloc::Layout::from_size_align(AP_STACK_BYTES, 16) {
            Ok(layout) => layout,
            Err(_) => return Err(SmpBringUpError::ApStackAllocationFailed),
        };
        // SAFETY: The global allocator is initialized; the zeroed allocation
        // is intentional so an unconfigured slot can never point at garbage.
        let raw = unsafe { alloc::alloc::alloc_zeroed(layout) };
        if raw.is_null() {
            return Err(SmpBringUpError::ApStackAllocationFailed);
        }
        // SAFETY: The raw allocation spans exactly AP_STACK_BYTES.
        unsafe {
            SYSTEMES_AP_STACKS[apic_id] =
                (raw as u64) + AP_STACK_BYTES as u64 - 16;
        }
    }

    for index in 0..candidate_count {
        let apic_id = candidate_ids[index];
        let slot = apic_id as usize;
        AP_STATES[slot].store(0, Ordering::Release);
        // SAFETY: The Local APIC is mapped and the trampoline page holds the
        // patched payload; only one AP is inside the trampoline at a time.
        unsafe {
            crate::apic::send_init_and_startup_ipi(apic_id, trampoline_page)
                .map_err(|_| SmpBringUpError::ApicUnavailable)?;
        }
        // The AP can already be parked by the time the bootstrap CPU polls,
        // so the start wait accepts any later state rather than exactly one.
        if !wait_for_slot_state_at_least(slot, SLOT_STATE_STARTED, AP_START_TIMEOUT_ITERATIONS) {
            return Err(SmpBringUpError::ApDidNotStart { apic_id });
        }
    }

    // Join the same validation on the bootstrap CPU while APs still run.
    run_core_validation(bsp_apic_id as usize);

    for index in 0..candidate_count {
        let apic_id = candidate_ids[index];
        let slot = apic_id as usize;
        if !wait_for_slot_state(slot, SLOT_STATE_PARKED, AP_PARK_TIMEOUT_ITERATIONS) {
            return Err(SmpBringUpError::ApDidNotPark { apic_id });
        }
    }

    // Every CPU increments both counters once per round.
    let cpu_count = 1 + candidate_count as u64;
    let expected_shared = cpu_count * VALIDATION_ROUNDS;
    let observed_shared = SHARED_RELAXED_COUNTER.load(Ordering::Acquire);
    let observed_locked = *SHARED_LOCKED_COUNTER.lock();
    if observed_shared != expected_shared || observed_locked != expected_shared {
        return Err(SmpBringUpError::CrossCoreCounterMismatch);
    }
    for index in 0..candidate_count {
        let slot = candidate_ids[index] as usize;
        if AP_PRIVATE_COUNTERS[slot].load(Ordering::Acquire) != VALIDATION_ROUNDS {
            return Err(SmpBringUpError::CrossCoreCounterMismatch);
        }
    }
    if AP_PRIVATE_COUNTERS[bsp_apic_id as usize].load(Ordering::Acquire) != VALIDATION_ROUNDS {
        return Err(SmpBringUpError::CrossCoreCounterMismatch);
    }

    let mut started_apic_ids = [0u8; MAX_CPUS_PER_APIC_ID];
    started_apic_ids[..candidate_count].copy_from_slice(&candidate_ids[..candidate_count]);
    Ok(SmpSummary {
        discovered_count: candidate_count,
        started_apic_ids,
        started_count: candidate_count,
        bsp_apic_id,
        trampoline_page,
        shared_relaxed_total: observed_shared,
        spin_locked_total: observed_locked,
    })
}

/// Entry far-jumped to from the 32-bit trampoline stage; creates a real stack.
///
/// # Safety
///
/// Only the trampoline may jump here, with interrupts off and RDI holding
/// this CPU's Local APIC ID.
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "sysv64" fn systemes_ap_entry() -> ! {
    core::arch::naked_asm!(
        // The trampoline's page switches the assembler to 16/32-bit modes;
        // restate the kernel's 64-bit mode for this function body.
        ".code64",
        // RDI holds the APIC ID passed from the trampoline's MMIO read.
        // naked_asm! assembles Intel syntax, so registers carry no '%'.
        "mov rax, rdi",
        "shl rax, 3",
        "lea rdx, [rip + SYSTEMES_AP_STACKS]",
        "mov rsp, qword ptr [rdx + rax]",
        // Stacks are published as ptr+size-16 and allocations are 16-aligned,
        // so RSP is 16-byte aligned here and SysV keeps it aligned modulo the
        // call's return-address push.
        "call systemes_ap_main_rust",
    ".Lsystemes_ap_main_park:",
        "cli",
        "hlt",
        "jmp .Lsystemes_ap_main_park",
    )
}

/// Rust body for an application processor running on its own stack.
///
/// # Safety
///
/// Only `systemes_ap_entry` may call this; RDI carries the APIC ID and the
/// CPU must arrive with interrupts disabled and a parked-slot stack.
#[unsafe(no_mangle)]
unsafe extern "sysv64" fn systemes_ap_main_rust(apic_id: u64) -> ! {
    let slot = apic_id as usize;
    // A stale slot state would mean the bootstrap CPU reused this slot.
    // SAFETY: The BSP initializes the per-AP stack before sending SIPI, and
    // this function runs only on the target AP with interrupts off.
    let stack_ok = unsafe { slot < MAX_CPUS_PER_APIC_ID && SYSTEMES_AP_STACKS[slot] != 0 };
    if !stack_ok {
        park_forever();
    }
    AP_STATES[slot].store(SLOT_STATE_STARTED, Ordering::Release);

    // P1-6b: Initialize per-CPU GDT/IDT/TSS on this AP.
    if slot < MAX_CPUS {
        // SAFETY: This CPU owns its slot and interrupts are disabled.
        unsafe { initialize_cpu(slot) };
    }

    // P1-6b/P1-6c: Enable this AP's Local APIC from the BSP-published ACPI
    // mapping, then program its timer using the BSP's calibrated initial count.
    let initial_count = get_timer_initial_count();
    if initial_count == 0 {
        park_forever();
    }
    // SAFETY: Local APIC is already initialized on this CPU via the trampoline.
    if unsafe { initialize_local_timer_on_cpu(initial_count) }.is_err() {
        park_forever();
    }

    run_core_validation(slot);

    // P1-6c: every AP owns an initialized scheduler before it is reported online.
    // The boot task is the AP's idle context; it can receive stolen work on a timer tick.
    if crate::scheduler::initialize_cpu(slot, &[]).is_err() {
        park_forever();
    }
    crate::scheduler::enable_preemption();
    AP_STATES[slot].store(SLOT_STATE_PARKED, Ordering::Release);
    crate::scheduler::run_cpu_loop()
}

/// Park the CPU with interrupts disabled until the next phase reclaims it.
fn park_forever() -> ! {
    loop {
        // SAFETY: A parked AP halts without touching memory.
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trampoline_payload_fits_its_page() {
        let bytes = trampoline_bytes();
        assert!(bytes.len() > 64);
        assert!(bytes.len() <= TRAMPOLINE_MAX_BYTES);
    }

    #[test]
    fn every_patch_magic_appears_exactly_once() {
        let bytes = trampoline_bytes();
        assert!(verify_unique_magics(bytes).is_ok());
    }

    #[test]
    fn patch_sites_relocate_provided_values() {
        let mut scratch = [0u8; 16];
        scratch[..4].copy_from_slice(&0x1122_3344u32.to_le_bytes());
        patch_u32(&mut scratch, 0x1122_3344, 0xAABB_CCD1).unwrap();
        assert_eq!(&scratch[..4], &0xAABB_CCD1u32.to_le_bytes());

        scratch[4..12].copy_from_slice(&0x5FAA_BBCC_DD11_2233u64.to_le_bytes());
        patch_u64(&mut scratch, 0x5FAA_BBCC_DD11_2233, 0x007E_0000_0000_1234).unwrap();
        assert_eq!(
            &scratch[4..12],
            &0x007E_0000_0000_1234u64.to_le_bytes()
        );
    }

    #[test]
    fn descriptor_patch_encodes_page_inside_24_bits() {
        let mut scratch = [0u8; 8];
        scratch[..4].copy_from_slice(&0x4C44_4702u32.to_le_bytes());
        patch_descriptor_base(&mut scratch, 0x4C44_4702, 0x7E000, 0x9A).unwrap();
        assert_eq!(&scratch[..4], &0x9A07_E000u32.to_le_bytes());
        assert_eq!(
            patch_descriptor_base(&mut scratch, 0x4C44_4702, 0x100_0000, 0x9A).err(),
            Some(SmpBringUpError::AmbiguousPatchSite)
        );
    }

    #[test]
    fn missing_or_ambiguous_magics_are_rejected() {
        assert_eq!(
            find_patch_site(&[0, 1, 2, 3], &[9, 9]).err(),
            Some(SmpBringUpError::MissingPatchSite)
        );
        assert_eq!(
            find_patch_site(&[9, 9, 0, 0, 9, 9], &[9, 9]).err(),
            Some(SmpBringUpError::AmbiguousPatchSite)
        );
        assert_eq!(
            find_patch_site(&[0, 9, 9, 0], &[9, 9]),
            Ok(1)
        );
    }
}
