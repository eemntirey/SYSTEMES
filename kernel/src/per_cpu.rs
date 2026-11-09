//! Per-CPU architecture for SMP: GDT, IDT, TSS, and scheduler state per logical CPU.
//!
//! This module implements the P1-6b milestone: each CPU gets its own descriptor
//! tables, interrupt stacks, Local APIC timer, and scheduler run queue. The
//! bootstrap CPU initializes its structures first, then each AP initializes its
//! own during bring-up before entering the scheduler.

use core::arch::asm;
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// Maximum number of logical CPUs supported (matches MADT limit).
pub const MAX_CPUS: usize = 64;

/// Stack size for per-CPU kernel privilege stacks.
const INTERRUPT_STACK_SIZE: usize = 16 * 1024;

/// GDT entry count: null, kernel code, kernel data, TSS low, TSS high, user data, user code.
const GDT_ENTRY_COUNT: usize = 7;

/// IDT entry count (full 256 vectors).
const IDT_ENTRY_COUNT: usize = 256;

/// Kernel segment selectors (identical across CPUs).
pub const KERNEL_CODE_SELECTOR: u16 = 0x08;
pub const KERNEL_DATA_SELECTOR: u16 = 0x10;
pub const TSS_SELECTOR: u16 = 0x18;
pub const USER_DATA_SELECTOR: u16 = 0x28;
pub const USER_CODE_SELECTOR: u16 = 0x30;

/// Exception gate attributes (DPL 0, present, interrupt gate).
const EXCEPTION_GATE_ATTRIBUTES: u8 = 0x8E;
/// User interrupt gate attributes (DPL 3, present, interrupt gate).
const USER_INTERRUPT_GATE_ATTRIBUTES: u8 = 0xEE;

/// User segment descriptors (flat, DPL 3).
const USER_DATA_DESCRIPTOR: u64 = 0x00CF_F200_0000_FFFF;
const USER_CODE_DESCRIPTOR: u64 = 0x00AF_FA00_0000_FFFF;

/// CPU exception vectors that push an error code.
const EXCEPTION_VECTOR_COUNT: usize = 32;

/// Local APIC timer vector.
const TIMER_VECTOR: u8 = 0x40;
/// Scheduler yield vector.
const YIELD_VECTOR: u8 = 0x41;
/// Spurious interrupt vector.
const SPURIOUS_VECTOR: u8 = 0xFF;
/// User-mode exit gate vector.
const USER_EXIT_VECTOR: u8 = 0x80;

#[repr(C, packed)]
struct TaskStateSegment {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    reserved2: u64,
    reserved3: u16,
    iomap_base: u16,
}

#[repr(align(4096))]
struct InterruptStack([u8; INTERRUPT_STACK_SIZE]);

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_middle: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const MISSING: Self = Self {
        offset_low: 0,
        selector: 0,
        ist: 0,
        attributes: 0,
        offset_middle: 0,
        offset_high: 0,
        reserved: 0,
    };

    const fn interrupt_gate(handler: u64, ist: u8) -> Self {
        Self::gate(handler, ist, EXCEPTION_GATE_ATTRIBUTES)
    }

    const fn user_interrupt_gate(handler: u64) -> Self {
        Self::gate(handler, 0, USER_INTERRUPT_GATE_ATTRIBUTES)
    }

    const fn gate(handler: u64, ist: u8, attributes: u8) -> Self {
        Self {
            offset_low: handler as u16,
            selector: KERNEL_CODE_SELECTOR,
            ist: ist & 0x07,
            attributes,
            offset_middle: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

const fn encode_tss_descriptor(base: u64, limit: u32) -> [u64; 2] {
    let low = (limit as u64 & 0xFFFF)
        | ((base & 0x00FF_FFFF) << 16)
        | (0x89u64 << 40)
        | ((((limit as u64) >> 16) & 0x0F) << 48)
        | (((base >> 24) & 0xFF) << 56);
    [low, base >> 32]
}

/// Per-CPU data structure containing all CPU-local state.
#[repr(align(4096))]
pub struct Cpu {
    /// This CPU's Local APIC ID.
    apic_id: u8,
    /// Whether this CPU has been fully initialized.
    initialized: AtomicBool,
    /// GDT entries (7 entries + padding for alignment).
    gdt: [u64; GDT_ENTRY_COUNT],
    /// IDT entries (256 vectors).
    idt: [IdtEntry; IDT_ENTRY_COUNT],
    /// Task State Segment.
    tss: TaskStateSegment,
    /// Privilege stack (RSP0).
    privilege_stack: InterruptStack,
    /// Double fault stack (IST1).
    double_fault_stack: InterruptStack,
    /// NMI stack (IST2).
    nmi_stack: InterruptStack,
    /// Machine check stack (IST3).
    machine_check_stack: InterruptStack,
    /// Syscall user RSP (saved on SYSCALL entry).
    syscall_user_rsp: AtomicU64,
    /// Syscall exit status.
    syscall_exit_status: AtomicU64,
    /// Current task index for this CPU's scheduler.
    current_task: AtomicUsize,
    /// Task count for this CPU.
    task_count: AtomicUsize,
    /// Whether preemption is enabled on this CPU.
    preemptive_enabled: AtomicBool,
    /// Preemption counter for this CPU.
    preemption_count: AtomicUsize,
}

impl Cpu {
    /// Create a new uninitialized CPU structure.
    const fn new() -> Self {
        Self {
            apic_id: 0,
            initialized: AtomicBool::new(false),
            gdt: [0; GDT_ENTRY_COUNT],
            idt: [IdtEntry::MISSING; IDT_ENTRY_COUNT],
            tss: TaskStateSegment {
                reserved0: 0,
                rsp0: 0,
                rsp1: 0,
                rsp2: 0,
                reserved1: 0,
                ist1: 0,
                ist2: 0,
                ist3: 0,
                ist4: 0,
                ist5: 0,
                ist6: 0,
                ist7: 0,
                reserved2: 0,
                reserved3: 0,
                iomap_base: core::mem::size_of::<TaskStateSegment>() as u16,
            },
            privilege_stack: InterruptStack([0; INTERRUPT_STACK_SIZE]),
            double_fault_stack: InterruptStack([0; INTERRUPT_STACK_SIZE]),
            nmi_stack: InterruptStack([0; INTERRUPT_STACK_SIZE]),
            machine_check_stack: InterruptStack([0; INTERRUPT_STACK_SIZE]),
            syscall_user_rsp: AtomicU64::new(0),
            syscall_exit_status: AtomicU64::new(0),
            current_task: AtomicUsize::new(0),
            task_count: AtomicUsize::new(0),
            preemptive_enabled: AtomicBool::new(false),
            preemption_count: AtomicUsize::new(0),
        }
    }

    /// Initialize this CPU's GDT, IDT, TSS, and stacks.
    ///
    /// # Safety
    /// Must be called once per CPU with interrupts disabled. The CPU must have
    /// its Local APIC enabled and mapped. The page table root must be active.
    pub unsafe fn initialize(&mut self, apic_id: u8) {
        self.apic_id = apic_id;

        // Initialize interrupt stacks.
        let rsp0_top = unsafe {
            addr_of_mut!(self.privilege_stack.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64
        };
        let df_top = unsafe {
            addr_of_mut!(self.double_fault_stack.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64
        };
        let nmi_top = unsafe {
            addr_of_mut!(self.nmi_stack.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64
        };
        let mc_top = unsafe {
            addr_of_mut!(self.machine_check_stack.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64
        };

        // Initialize TSS.
        let tss = addr_of_mut!(self.tss);
        unsafe {
            core::ptr::addr_of_mut!((*tss).rsp0).write_unaligned(rsp0_top);
            core::ptr::addr_of_mut!((*tss).ist1).write_unaligned(df_top);
            core::ptr::addr_of_mut!((*tss).ist2).write_unaligned(nmi_top);
            core::ptr::addr_of_mut!((*tss).ist3).write_unaligned(mc_top);
            core::ptr::addr_of_mut!((*tss).iomap_base)
                .write_unaligned(core::mem::size_of::<TaskStateSegment>() as u16);
        }

        // Install TSS descriptor in GDT (entries 3 and 4).
        let tss_base = tss as u64;
        let [tss_low, tss_high] = encode_tss_descriptor(tss_base, (core::mem::size_of::<TaskStateSegment>() - 1) as u32);
        let gdt = addr_of_mut!(self.gdt).cast::<u64>();
        unsafe {
            gdt.add(0).write(0); // null
            gdt.add(1).write(0x00AF_9A00_0000_FFFF); // kernel code
            gdt.add(2).write(0x00CF_9200_0000_FFFF); // kernel data
            gdt.add(3).write(tss_low);
            gdt.add(4).write(tss_high);
            gdt.add(5).write(USER_DATA_DESCRIPTOR);
            gdt.add(6).write(USER_CODE_DESCRIPTOR);
        }

        // Load GDT.
        let gdt_pointer = DescriptorTablePointer {
            limit: (GDT_ENTRY_COUNT * core::mem::size_of::<u64>() - 1) as u16,
            base: gdt as u64,
        };
        unsafe { systemes_load_gdt(&gdt_pointer) };

        // Load TR.
        // SAFETY: The TSS descriptor at selector 0x18 is present and references this CPU's static TSS.
        unsafe { systemes_load_tr(TSS_SELECTOR) };

        // Install exception handlers (vectors 0-31).
        let handlers: [usize; EXCEPTION_VECTOR_COUNT] = [
            systemes_isr0 as *const () as usize,
            systemes_isr1 as *const () as usize,
            systemes_isr2 as *const () as usize,
            systemes_isr3 as *const () as usize,
            systemes_isr4 as *const () as usize,
            systemes_isr5 as *const () as usize,
            systemes_isr6 as *const () as usize,
            systemes_isr7 as *const () as usize,
            systemes_isr8 as *const () as usize,
            systemes_isr9 as *const () as usize,
            systemes_isr10 as *const () as usize,
            systemes_isr11 as *const () as usize,
            systemes_isr12 as *const () as usize,
            systemes_isr13 as *const () as usize,
            systemes_isr14 as *const () as usize,
            systemes_isr15 as *const () as usize,
            systemes_isr16 as *const () as usize,
            systemes_isr17 as *const () as usize,
            systemes_isr18 as *const () as usize,
            systemes_isr19 as *const () as usize,
            systemes_isr20 as *const () as usize,
            systemes_isr21 as *const () as usize,
            systemes_isr22 as *const () as usize,
            systemes_isr23 as *const () as usize,
            systemes_isr24 as *const () as usize,
            systemes_isr25 as *const () as usize,
            systemes_isr26 as *const () as usize,
            systemes_isr27 as *const () as usize,
            systemes_isr28 as *const () as usize,
            systemes_isr29 as *const () as usize,
            systemes_isr30 as *const () as usize,
            systemes_isr31 as *const () as usize,
        ];
        let idt = addr_of_mut!(self.idt) as *mut IdtEntry;
        for (vector, handler) in handlers.iter().enumerate() {
            let ist = match vector {
                8 => 1,  // Double fault -> IST1
                2 => 2,  // NMI -> IST2
                18 => 3, // Machine check -> IST3
                _ => 0,
            };
            unsafe { idt.add(vector).write(IdtEntry::interrupt_gate(*handler as u64, ist)) };
        }

        // Install Local APIC timer, yield, spurious, and user exit gates.
        unsafe {
            idt.add(TIMER_VECTOR as usize).write(IdtEntry::interrupt_gate(systemes_lapic_timer as *const () as usize as u64, 0));
            idt.add(YIELD_VECTOR as usize).write(IdtEntry::interrupt_gate(systemes_scheduler_yield as *const () as usize as u64, 0));
            idt.add(SPURIOUS_VECTOR as usize).write(IdtEntry::interrupt_gate(systemes_apic_spurious as *const () as usize as u64, 0));
            idt.add(USER_EXIT_VECTOR as usize).write(IdtEntry::user_interrupt_gate(systemes_user_exit as *const () as usize as u64));
        }

        // Load IDT.
        let idt_pointer = DescriptorTablePointer {
            limit: (IDT_ENTRY_COUNT * core::mem::size_of::<IdtEntry>() - 1) as u16,
            base: idt as u64,
        };
        unsafe {
            asm!(
                "lidt [{}]",
                in(reg) &idt_pointer,
                options(readonly, nostack, preserves_flags)
            )
        };

        // Map the trampoline page as RX in this CPU's page tables (shared).
        // The trampoline page is already mapped globally by the BSP.

        self.initialized.store(true, Ordering::Release);
    }

    /// Initialize this CPU's Local APIC timer (BSP only - does full calibration).
    ///
    /// # Safety
    /// Must be called after `initialize()` with the Local APIC already mapped.
    pub unsafe fn initialize_local_timer(&mut self, target_hz: u32) -> Result<TimerCalibration, ApicInitError> {
        // The Local APIC base is per-CPU in x2APIC but shared in xAPIC.
        // For now we use the shared mapping from the BSP.
        // TODO: Per-CPU Local APIC MMIO mapping if x2APIC is used.
        unsafe { crate::apic::calibrate_and_initialize_local_timer(target_hz) }
    }

    /// Initialize this CPU's Local APIC timer using pre-calibrated count (for APs).
    ///
    /// # Safety
    /// Must be called after `initialize()` with the Local APIC already mapped.
    pub unsafe fn initialize_ap_timer(&mut self) -> Result<(), ApicInitError> {
        unsafe { crate::apic::initialize_local_timer_on_cpu(crate::apic::get_timer_initial_count()) }
    }

    /// Get this CPU's APIC ID.
    pub fn apic_id(&self) -> u8 {
        self.apic_id
    }

    /// Check if this CPU is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }

    /// Set current task for this CPU.
    pub fn set_current_task(&self, task: usize) {
        self.current_task.store(task, Ordering::Release);
    }

    /// Get current task for this CPU.
    pub fn current_task(&self) -> usize {
        self.current_task.load(Ordering::Acquire)
    }

    /// Set task count for this CPU.
    pub fn set_task_count(&self, count: usize) {
        self.task_count.store(count, Ordering::Release);
    }

    /// Get task count for this CPU.
    pub fn task_count(&self) -> usize {
        self.task_count.load(Ordering::Acquire)
    }

    /// Enable preemption on this CPU.
    pub fn enable_preemption(&self) {
        self.preemptive_enabled.store(true, Ordering::Release);
    }

    /// Disable preemption on this CPU.
    pub fn disable_preemption(&self) {
        self.preemptive_enabled.store(false, Ordering::Release);
    }

    /// Check if preemption is enabled.
    pub fn is_preemption_enabled(&self) -> bool {
        self.preemptive_enabled.load(Ordering::Acquire)
    }

    /// Increment preemption counter.
    pub fn increment_preemption_count(&self) {
        self.preemption_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get preemption count.
    pub fn preemption_count(&self) -> usize {
        self.preemption_count.load(Ordering::Acquire)
    }

    /// Get syscall exit status.
    pub fn syscall_exit_status(&self) -> u64 {
        self.syscall_exit_status.load(Ordering::Acquire)
    }

    /// Set syscall exit status.
    pub fn set_syscall_exit_status(&self, status: u64) {
        self.syscall_exit_status.store(status, Ordering::Release);
    }

    /// Get syscall user RSP.
    pub fn syscall_user_rsp(&self) -> u64 {
        self.syscall_user_rsp.load(Ordering::Acquire)
    }

    /// Set syscall user RSP.
    pub fn set_syscall_user_rsp(&self, rsp: u64) {
        self.syscall_user_rsp.store(rsp, Ordering::Release);
    }
}

/// Global array of per-CPU structures.
static mut CPUS: [Cpu; MAX_CPUS] = [const { Cpu::new() }; MAX_CPUS];

/// Current CPU index (set by each CPU on entry).
static CURRENT_CPU_INDEX: AtomicUsize = AtomicUsize::new(0);

/// Get the current CPU's structure.
///
/// # Safety
/// Must only be called after the current CPU has initialized its per-CPU data.
pub fn current_cpu() -> &'static Cpu {
    let index = CURRENT_CPU_INDEX.load(Ordering::Acquire);
    assert!(index < MAX_CPUS, "CPU index out of bounds");
    // SAFETY: The CPU array is static and the index is validated.
    unsafe { &*core::ptr::addr_of!(CPUS).cast::<Cpu>().add(index) }
}

/// Get a mutable reference to the current CPU's structure.
///
/// # Safety
/// Must only be called after the current CPU has initialized its per-CPU data.
pub fn current_cpu_mut() -> &'static mut Cpu {
    let index = CURRENT_CPU_INDEX.load(Ordering::Acquire);
    assert!(index < MAX_CPUS, "CPU index out of bounds");
    // SAFETY: The CPU array is static and the index is validated.
    unsafe { &mut *core::ptr::addr_of_mut!(CPUS).cast::<Cpu>().add(index) }
}

/// Get a CPU structure by APIC ID.
///
/// # Safety
/// The caller must ensure the CPU at `apic_id` has been initialized.
pub fn cpu_by_apic_id(apic_id: u8) -> Option<&'static Cpu> {
    if apic_id as usize >= MAX_CPUS {
        return None;
    }
    let cpu = unsafe { &*core::ptr::addr_of!(CPUS).cast::<Cpu>().add(apic_id as usize) };
    if cpu.is_initialized() {
        Some(cpu)
    } else {
        None
    }
}

/// Get a mutable CPU structure by APIC ID.
///
/// # Safety
/// The caller must ensure the CPU at `apic_id` has been initialized.
pub fn cpu_by_apic_id_mut(apic_id: u8) -> Option<&'static mut Cpu> {
    if apic_id as usize >= MAX_CPUS {
        return None;
    }
    let cpu = unsafe { &mut *core::ptr::addr_of_mut!(CPUS).cast::<Cpu>().add(apic_id as usize) };
    if cpu.is_initialized() {
        Some(cpu)
    } else {
        None
    }
}

/// Initialize the bootstrap CPU's per-CPU data.
///
/// # Safety
/// Must be called once on the BSP before any AP is started.
pub unsafe fn initialize_bsp() -> Result<(), ApicInitError> {
    let bsp_apic_id = crate::apic::current_apic_id().ok_or(ApicInitError::CpuApicUnsupported)?;
    let index = bsp_apic_id as usize;
    if index >= MAX_CPUS {
        return Err(ApicInitError::InvalidPhysicalAddress);
    }
    CURRENT_CPU_INDEX.store(index, Ordering::Release);
    let cpu = unsafe { &mut *core::ptr::addr_of_mut!(CPUS).cast::<Cpu>().add(index) };
    unsafe { cpu.initialize(bsp_apic_id) };
    unsafe { cpu.initialize_local_timer(100)? };
    Ok(())
}

/// Initialize an application processor's per-CPU data.
///
/// # Safety
/// Called by the AP itself after jumping to `systemes_ap_entry`.
pub unsafe fn initialize_ap(apic_id: u8) -> Result<(), ApicInitError> {
    let index = apic_id as usize;
    if index >= MAX_CPUS {
        return Err(ApicInitError::InvalidPhysicalAddress);
    }
    CURRENT_CPU_INDEX.store(index, Ordering::Release);
    let cpu = unsafe { &mut *core::ptr::addr_of_mut!(CPUS).cast::<Cpu>().add(index) };
    unsafe { cpu.initialize(apic_id) };
    unsafe { cpu.initialize_ap_timer()? };
    Ok(())
}

/// Called by the AP entry assembly to enter the Rust per-CPU initialization.
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "sysv64" fn systemes_ap_entry_per_cpu() -> ! {
    core::arch::naked_asm!(
        ".code64",
        "mov rax, rdi",
        "call systemes_ap_main_rust_per_cpu",
    ".Lsystemes_ap_main_park:",
        "cli",
        "hlt",
        "jmp .Lsystemes_ap_main_park",
    )
}

/// Rust body for AP per-CPU initialization and scheduler entry.
#[unsafe(no_mangle)]
unsafe extern "sysv64" fn systemes_ap_main_rust_per_cpu(apic_id: u64) -> ! {
    let apic_id = apic_id as u8;
    // Initialize per-CPU GDT, IDT, TSS, and Local APIC timer.
    if unsafe { initialize_ap(apic_id) }.is_err() {
        // Initialization failed, park forever.
        loop {
            unsafe { asm!("cli; hlt", options(nomem, nostack)) };
        }
    }

    // Mark this CPU as ready in the global state.
    crate::smp::mark_ap_started(apic_id);

    // Enter the scheduler loop on this CPU.
    crate::scheduler::run_cpu_loop();
}

// External assembly functions from interrupts.rs
unsafe extern "C" {
    fn systemes_load_gdt(pointer: *const DescriptorTablePointer);
    fn systemes_load_tr(selector: u16);
    fn systemes_enter_ring3(user_rip: u64, user_rsp: u64, user_rflags: u64);
    fn systemes_ring3_exit_trampoline();
    fn systemes_isr0();
    fn systemes_isr1();
    fn systemes_isr2();
    fn systemes_isr3();
    fn systemes_isr4();
    fn systemes_isr5();
    fn systemes_isr6();
    fn systemes_isr7();
    fn systemes_isr8();
    fn systemes_isr9();
    fn systemes_isr10();
    fn systemes_isr11();
    fn systemes_isr12();
    fn systemes_isr13();
    fn systemes_isr14();
    fn systemes_isr15();
    fn systemes_isr16();
    fn systemes_isr17();
    fn systemes_isr18();
    fn systemes_isr19();
    fn systemes_isr20();
    fn systemes_isr21();
    fn systemes_isr22();
    fn systemes_isr23();
    fn systemes_isr24();
    fn systemes_isr25();
    fn systemes_isr26();
    fn systemes_isr27();
    fn systemes_isr28();
    fn systemes_isr29();
    fn systemes_isr30();
    fn systemes_isr31();
    fn systemes_lapic_timer();
    fn systemes_scheduler_yield();
    fn systemes_apic_spurious();
    fn systemes_user_exit();
    fn systemes_syscall_entry();
}

// Re-export timer calibration from apic
pub use crate::apic::{TimerCalibration, ApicInitError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_structure_size_is_reasonable() {
        // Cpu includes 5 interrupt stacks (5 * 16KB = 80KB), IDT (4KB), GDT, TSS, etc.
        // Total should fit in a reasonable number of pages.
        let size = core::mem::size_of::<Cpu>();
        assert!(size <= 128 * 1024, "Cpu size {} exceeds 128KB budget", size);
        assert!(core::mem::align_of::<Cpu>() >= 4096);
    }

    #[test]
    fn gdt_entry_count_matches_constants() {
        assert_eq!(GDT_ENTRY_COUNT, 7);
    }

    #[test]
    fn idt_entry_count_is_256() {
        assert_eq!(IDT_ENTRY_COUNT, 256);
    }
}