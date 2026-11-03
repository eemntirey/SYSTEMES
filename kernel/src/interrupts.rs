//! x86_64 descriptor tables and fatal CPU-exception diagnostics.
//!
//! The kernel remains in the UEFI-provided long-mode environment. This module
//! installs its own GDT and IDT but deliberately does not enable hardware IRQs.

use core::arch::{asm, global_asm};
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use crate::{acpi::MadtInfo, memory::PhysicalFrameAllocator};

const KERNEL_CODE_SELECTOR: u16 = 0x08;
const TSS_SELECTOR: u16 = 0x18;
const EXCEPTION_GATE_ATTRIBUTES: u8 = 0x8E;
const EXCEPTION_VECTOR_COUNT: usize = 32;
const IRQ_VECTOR_BASE: usize = 32;
const IRQ_COUNT: usize = 16;
const ACTIVE_VECTOR_COUNT: usize = IRQ_VECTOR_BASE + IRQ_COUNT;
const PIC_MASTER_COMMAND: u16 = 0x20;
const PIC_MASTER_DATA: u16 = 0x21;
const PIC_SLAVE_COMMAND: u16 = 0xA0;
const PIC_SLAVE_DATA: u16 = 0xA1;
const PIT_CHANNEL0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;
const PIT_INPUT_HZ: u32 = 1_193_182;
const PIT_FREQUENCY_HZ: u32 = 100;
const PIT_DIVISOR: u16 = (PIT_INPUT_HZ / PIT_FREQUENCY_HZ) as u16;
static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);
static IRQ_COUNTS: [AtomicU64; IRQ_COUNT] = [const { AtomicU64::new(0) }; IRQ_COUNT];
static APIC_SPURIOUS_COUNT: AtomicU64 = AtomicU64::new(0);
static PIC_SPURIOUS_7_COUNT: AtomicU64 = AtomicU64::new(0);
static PIC_SPURIOUS_15_COUNT: AtomicU64 = AtomicU64::new(0);
static APIC_ACTIVE: AtomicBool = AtomicBool::new(false);
static LOCAL_APIC_BASE: AtomicUsize = AtomicUsize::new(0);
const IA32_APIC_BASE_MSR: u32 = 0x1B;
const APIC_BASE_MASK: u64 = 0x000F_FFFF_FFFF_F000;
const APIC_ENABLE: u64 = 1 << 11;
const APIC_X2_ENABLE: u64 = 1 << 10;
const CPUID_APIC: u32 = 1 << 9;
const APIC_SPURIOUS_VECTOR: u32 = 0xFF;
const APIC_TIMER_VECTOR: u8 = IRQ_VECTOR_BASE as u8;
const IOAPIC_REG_VERSION: u8 = 0x01;
const IOAPIC_REG_REDIR_BASE: u8 = 0x10;

const INTERRUPT_STACK_SIZE: usize = 16 * 1024;

static mut GDT: [u64; 5] = [
    0,
    0x00AF_9A00_0000_FFFF,
    0x00CF_9200_0000_FFFF,
    0,
    0,
];

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

static mut TSS: TaskStateSegment = TaskStateSegment {
    reserved0: 0, rsp0: 0, rsp1: 0, rsp2: 0, reserved1: 0,
    ist1: 0, ist2: 0, ist3: 0, ist4: 0, ist5: 0, ist6: 0, ist7: 0,
    reserved2: 0, reserved3: 0, iomap_base: core::mem::size_of::<TaskStateSegment>() as u16,
};
static mut PRIVILEGE_STACK: InterruptStack = InterruptStack([0; INTERRUPT_STACK_SIZE]);
static mut DOUBLE_FAULT_STACK: InterruptStack = InterruptStack([0; INTERRUPT_STACK_SIZE]);
static mut NMI_STACK: InterruptStack = InterruptStack([0; INTERRUPT_STACK_SIZE]);
static mut MACHINE_CHECK_STACK: InterruptStack = InterruptStack([0; INTERRUPT_STACK_SIZE]);

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

const fn encode_tss_descriptor(base: u64, limit: u32) -> [u64; 2] {
    let low = (limit as u64 & 0xFFFF)
        | ((base & 0x00FF_FFFF) << 16)
        | (0x89u64 << 40)
        | ((((limit as u64) >> 16) & 0x0F) << 48)
        | (((base >> 24) & 0xFF) << 56);
    [low, base >> 32]
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
        Self {
            offset_low: handler as u16,
            selector: KERNEL_CODE_SELECTOR,
            ist: ist & 0x07,
            attributes: EXCEPTION_GATE_ATTRIBUTES,
            offset_middle: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::MISSING; 256];

unsafe extern "C" {
    fn systemes_load_gdt(pointer: *const DescriptorTablePointer);
    fn systemes_load_tr();
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
    fn systemes_irq0();
    fn systemes_irq1();
    fn systemes_irq2();
    fn systemes_irq3();
    fn systemes_irq4();
    fn systemes_irq5();
    fn systemes_irq6();
    fn systemes_irq7();
    fn systemes_irq8();
    fn systemes_irq9();
    fn systemes_irq10();
    fn systemes_irq11();
    fn systemes_irq12();
    fn systemes_irq13();
    fn systemes_irq14();
    fn systemes_irq15();
    fn systemes_apic_spurious();
}

global_asm!(r#"
.global systemes_load_gdt
.type systemes_load_gdt, @function
systemes_load_gdt:
    lgdt [rdi]
    push 0x08
    lea rax, [rip + .Lsystemes_reload_cs]
    push rax
    retfq
.Lsystemes_reload_cs:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    ret

.global systemes_load_tr
.type systemes_load_tr, @function
systemes_load_tr:
    mov ax, 0x18
    ltr ax
    ret

.global systemes_isr0
.type systemes_isr0, @function
systemes_isr0:
    push 0
    push 0
    jmp systemes_exception_common

.global systemes_isr1
.type systemes_isr1, @function
systemes_isr1:
    push 0
    push 1
    jmp systemes_exception_common

.global systemes_isr2
.type systemes_isr2, @function
systemes_isr2:
    push 0
    push 2
    jmp systemes_exception_common

.global systemes_isr3
.type systemes_isr3, @function
systemes_isr3:
    push 0
    push 3
    jmp systemes_exception_common

.global systemes_isr4
.type systemes_isr4, @function
systemes_isr4:
    push 0
    push 4
    jmp systemes_exception_common

.global systemes_isr5
.type systemes_isr5, @function
systemes_isr5:
    push 0
    push 5
    jmp systemes_exception_common

.global systemes_isr6
.type systemes_isr6, @function
systemes_isr6:
    push 0
    push 6
    jmp systemes_exception_common

.global systemes_isr7
.type systemes_isr7, @function
systemes_isr7:
    push 0
    push 7
    jmp systemes_exception_common

.global systemes_isr8
.type systemes_isr8, @function
systemes_isr8:
    push 8
    jmp systemes_exception_common

.global systemes_isr9
.type systemes_isr9, @function
systemes_isr9:
    push 0
    push 9
    jmp systemes_exception_common

.global systemes_isr10
.type systemes_isr10, @function
systemes_isr10:
    push 10
    jmp systemes_exception_common

.global systemes_isr11
.type systemes_isr11, @function
systemes_isr11:
    push 11
    jmp systemes_exception_common

.global systemes_isr12
.type systemes_isr12, @function
systemes_isr12:
    push 12
    jmp systemes_exception_common

.global systemes_isr13
.type systemes_isr13, @function
systemes_isr13:
    push 13
    jmp systemes_exception_common

.global systemes_isr14
.type systemes_isr14, @function
systemes_isr14:
    push 14
    jmp systemes_exception_common

.global systemes_isr15
.type systemes_isr15, @function
systemes_isr15:
    push 0
    push 15
    jmp systemes_exception_common

.global systemes_isr16
.type systemes_isr16, @function
systemes_isr16:
    push 0
    push 16
    jmp systemes_exception_common

.global systemes_isr17
.type systemes_isr17, @function
systemes_isr17:
    push 17
    jmp systemes_exception_common

.global systemes_isr18
.type systemes_isr18, @function
systemes_isr18:
    push 0
    push 18
    jmp systemes_exception_common

.global systemes_isr19
.type systemes_isr19, @function
systemes_isr19:
    push 0
    push 19
    jmp systemes_exception_common

.global systemes_isr20
.type systemes_isr20, @function
systemes_isr20:
    push 0
    push 20
    jmp systemes_exception_common

.global systemes_isr21
.type systemes_isr21, @function
systemes_isr21:
    push 21
    jmp systemes_exception_common

.global systemes_isr22
.type systemes_isr22, @function
systemes_isr22:
    push 0
    push 22
    jmp systemes_exception_common

.global systemes_isr23
.type systemes_isr23, @function
systemes_isr23:
    push 0
    push 23
    jmp systemes_exception_common

.global systemes_isr24
.type systemes_isr24, @function
systemes_isr24:
    push 0
    push 24
    jmp systemes_exception_common

.global systemes_isr25
.type systemes_isr25, @function
systemes_isr25:
    push 0
    push 25
    jmp systemes_exception_common

.global systemes_isr26
.type systemes_isr26, @function
systemes_isr26:
    push 0
    push 26
    jmp systemes_exception_common

.global systemes_isr27
.type systemes_isr27, @function
systemes_isr27:
    push 0
    push 27
    jmp systemes_exception_common

.global systemes_isr28
.type systemes_isr28, @function
systemes_isr28:
    push 0
    push 28
    jmp systemes_exception_common

.global systemes_isr29
.type systemes_isr29, @function
systemes_isr29:
    push 29
    jmp systemes_exception_common

.global systemes_isr30
.type systemes_isr30, @function
systemes_isr30:
    push 30
    jmp systemes_exception_common

.global systemes_isr31
.type systemes_isr31, @function
systemes_isr31:
    push 0
    push 31
    jmp systemes_exception_common

.global systemes_irq0
.type systemes_irq0, @function
systemes_irq0:
    push 0
    push 32
    jmp systemes_irq_common

.global systemes_irq1
.type systemes_irq1, @function
systemes_irq1:
    push 0
    push 33
    jmp systemes_irq_common

.global systemes_irq2
.type systemes_irq2, @function
systemes_irq2:
    push 0
    push 34
    jmp systemes_irq_common

.global systemes_irq3
.type systemes_irq3, @function
systemes_irq3:
    push 0
    push 35
    jmp systemes_irq_common

.global systemes_irq4
.type systemes_irq4, @function
systemes_irq4:
    push 0
    push 36
    jmp systemes_irq_common

.global systemes_irq5
.type systemes_irq5, @function
systemes_irq5:
    push 0
    push 37
    jmp systemes_irq_common

.global systemes_irq6
.type systemes_irq6, @function
systemes_irq6:
    push 0
    push 38
    jmp systemes_irq_common

.global systemes_irq7
.type systemes_irq7, @function
systemes_irq7:
    push 0
    push 39
    jmp systemes_irq_common

.global systemes_irq8
.type systemes_irq8, @function
systemes_irq8:
    push 0
    push 40
    jmp systemes_irq_common

.global systemes_irq9
.type systemes_irq9, @function
systemes_irq9:
    push 0
    push 41
    jmp systemes_irq_common

.global systemes_irq10
.type systemes_irq10, @function
systemes_irq10:
    push 0
    push 42
    jmp systemes_irq_common

.global systemes_irq11
.type systemes_irq11, @function
systemes_irq11:
    push 0
    push 43
    jmp systemes_irq_common

.global systemes_irq12
.type systemes_irq12, @function
systemes_irq12:
    push 0
    push 44
    jmp systemes_irq_common

.global systemes_irq13
.type systemes_irq13, @function
systemes_irq13:
    push 0
    push 45
    jmp systemes_irq_common

.global systemes_irq14
.type systemes_irq14, @function
systemes_irq14:
    push 0
    push 46
    jmp systemes_irq_common

.global systemes_irq15
.type systemes_irq15, @function
systemes_irq15:
    push 0
    push 47
    jmp systemes_irq_common

.global systemes_apic_spurious
.type systemes_apic_spurious, @function
systemes_apic_spurious:
    push 0
    push 255
    jmp systemes_irq_common

.global systemes_exception_common
.type systemes_exception_common, @function
systemes_exception_common:
    cld
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov rdi, qword ptr [rsp + 120]
    mov rsi, qword ptr [rsp + 128]
    and rsp, -16
    call systemes_exception_dispatch
.Lsystemes_exception_halt:
    cli
    hlt
    jmp .Lsystemes_exception_halt

.global systemes_irq_common
.type systemes_irq_common, @function
systemes_irq_common:
    cld
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov rdi, qword ptr [rsp + 120]
    mov r12, rsp
    and rsp, -16
    call systemes_irq_dispatch
    mov rsp, r12
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    add rsp, 16
    iretq
"#);

/// Return true only when the processor loaded the kernel's 64-bit TSS selector.
pub fn verify_task_register() -> bool {
    let selector: u16;
    // SAFETY: STR is available at CPL0 and reads the current task-register selector.
    unsafe { asm!("str {0:x}", out(reg) selector, options(nomem, nostack, preserves_flags)); }
    selector == TSS_SELECTOR
}

/// Validate the live TSS stack pointers and the IST selectors installed in the IDT.
pub fn verify_ist_configuration() -> bool {
    let tss = addr_of!(TSS);
    let idt = addr_of!(IDT) as *const IdtEntry;
    let rsp0 = unsafe { core::ptr::addr_of!((*tss).rsp0).read_unaligned() };
    let ist1 = unsafe { core::ptr::addr_of!((*tss).ist1).read_unaligned() };
    let ist2 = unsafe { core::ptr::addr_of!((*tss).ist2).read_unaligned() };
    let ist3 = unsafe { core::ptr::addr_of!((*tss).ist3).read_unaligned() };
    let iomap_base = unsafe { core::ptr::addr_of!((*tss).iomap_base).read_unaligned() };
    let df_gate_ist = unsafe { core::ptr::addr_of!((*idt.add(8)).ist).read_unaligned() };
    let nmi_gate_ist = unsafe { core::ptr::addr_of!((*idt.add(2)).ist).read_unaligned() };
    let mc_gate_ist = unsafe { core::ptr::addr_of!((*idt.add(18)).ist).read_unaligned() };

    let aligned_stack = |top: u64| top != 0 && top & 0x0F == 0;
    aligned_stack(rsp0)
        && aligned_stack(ist1)
        && aligned_stack(ist2)
        && aligned_stack(ist3)
        && iomap_base == core::mem::size_of::<TaskStateSegment>() as u16
        && (df_gate_ist, nmi_gate_ist, mc_gate_ist) == (1, 2, 3)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApicInitError {
    CpuApicUnsupported, X2ApicUnsupported, InvalidPhysicalAddress,
    MmioMappingFailed, InvalidIoApicVersion, Irq0GsiNotCovered,
    InvalidInterruptOverride,
    IoApicProgrammingFailed,
}

unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe { asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack)); }
    ((high as u64) << 32) | low as u64
}

unsafe fn write_msr(msr: u32, value: u64) {
    unsafe { asm!("wrmsr", in("ecx") msr, in("eax") value as u32, in("edx") (value >> 32) as u32, options(nomem, nostack)); }
}

unsafe fn lapic_write(base: *mut u8, offset: usize, value: u32) {
    unsafe { core::ptr::write_volatile(base.add(offset) as *mut u32, value); }
}

unsafe fn ioapic_read(base: *mut u8, register: u8) -> u32 {
    unsafe {
        core::ptr::write_volatile(base as *mut u32, register as u32);
        core::ptr::read_volatile(base.add(0x10) as *const u32)
    }
}

unsafe fn ioapic_write(base: *mut u8, register: u8, value: u32) {
    unsafe {
        core::ptr::write_volatile(base as *mut u32, register as u32);
        core::ptr::write_volatile(base.add(0x10) as *mut u32, value);
    }
}

/// Activate xAPIC and route ISA PIT IRQ0 through the matching MADT IOAPIC GSI.
pub unsafe fn initialize_apic_ioapic(
    madt: &MadtInfo,
    allocator: &mut PhysicalFrameAllocator,
    page_table_root: u64,
) -> Result<(), ApicInitError> {
    if unsafe { core::arch::x86_64::__cpuid(1) }.edx & CPUID_APIC == 0 {
        return Err(ApicInitError::CpuApicUnsupported);
    }
    if madt.io_apic_count == 0 || madt.local_apic_count == 0 {
        return Err(ApicInitError::InvalidPhysicalAddress);
    }
    let mut apic_base = unsafe { read_msr(IA32_APIC_BASE_MSR) };
    if apic_base & APIC_X2_ENABLE != 0 { return Err(ApicInitError::X2ApicUnsupported); }
    if madt.local_apic_address == 0 || madt.local_apic_address & 0xFFF != 0
        || madt.local_apic_address & !APIC_BASE_MASK != 0 {
        return Err(ApicInitError::InvalidPhysicalAddress);
    }
    apic_base = (apic_base & !(APIC_BASE_MASK | APIC_X2_ENABLE))
        | (madt.local_apic_address & APIC_BASE_MASK) | APIC_ENABLE;
    unsafe { write_msr(IA32_APIC_BASE_MSR, apic_base); }

    let lapic = unsafe {
        crate::paging::map_mmio_uncached(allocator, page_table_root, madt.local_apic_address, 4096)
    }.map_err(|_| ApicInitError::MmioMappingFailed)?;
    let lapic_id = unsafe { core::ptr::read_volatile(lapic.add(0x20) as *const u32) } >> 24;
    if lapic_id > 255 { return Err(ApicInitError::InvalidPhysicalAddress); }
    unsafe {
        lapic_write(lapic, 0x80, 0);
        for offset in [0x320usize, 0x330, 0x340, 0x350, 0x360, 0x370] {
            lapic_write(lapic, offset, 1 << 16);
        }
        lapic_write(lapic, 0xF0, (1 << 8) | APIC_SPURIOUS_VECTOR);
        lapic_write(lapic, 0xB0, 0);
    }

    let mut irq0_gsi = 0u32;
    let mut iso_flags = 0u16;
    for index in 0..madt.override_count {
        let iso = madt.overrides[index];
        if iso.bus == 0 && iso.source == 0 {
            if iso.flags & 0x3 == 2 || (iso.flags >> 2) & 0x3 == 2 {
                return Err(ApicInitError::InvalidInterruptOverride);
            }
            irq0_gsi = iso.gsi;
            iso_flags = iso.flags;
            break;
        }
    }

    let mut selected: Option<(*mut u8, u32)> = None;
    for index in 0..madt.io_apic_count {
        let io = madt.io_apics[index];
        if io.address == 0 || io.address & 0xFFF != 0
            || io.address as u64 >= crate::paging::IDENTITY_MAP_BYTES {
            return Err(ApicInitError::InvalidPhysicalAddress);
        }
        let base = unsafe {
            crate::paging::map_mmio_uncached(allocator, page_table_root, io.address as u64, 4096)
        }.map_err(|_| ApicInitError::MmioMappingFailed)?;
        let version = unsafe { ioapic_read(base, IOAPIC_REG_VERSION) };
        let max_redirection = (version >> 16) & 0xFF;
        if max_redirection == 0 || max_redirection > 119 {
            return Err(ApicInitError::InvalidIoApicVersion);
        }
        let last_gsi = io.gsi_base.checked_add(max_redirection).ok_or(ApicInitError::InvalidIoApicVersion)?;
        if irq0_gsi >= io.gsi_base && irq0_gsi <= last_gsi {
            selected = Some((base, irq0_gsi - io.gsi_base));
            break;
        }
    }
    let (io_base, redir_index) = selected.ok_or(ApicInitError::Irq0GsiNotCovered)?;
    let low_reg = IOAPIC_REG_REDIR_BASE.checked_add((redir_index * 2) as u8)
        .ok_or(ApicInitError::InvalidIoApicVersion)?;
    let high_reg = low_reg.checked_add(1).ok_or(ApicInitError::InvalidIoApicVersion)?;
    let mut low = APIC_TIMER_VECTOR as u32 | (1 << 16);
    if iso_flags & 0x3 == 3 { low |= 1 << 13; }
    if (iso_flags >> 2) & 0x3 == 3 { low |= 1 << 15; }
    unsafe {
        ioapic_write(io_base, high_reg, (lapic_id as u32) << 24);
        ioapic_write(io_base, low_reg, low);
        let programmed_low = ioapic_read(io_base, low_reg);
        let programmed_high = ioapic_read(io_base, high_reg);
        if (programmed_low & 0x1FFFF) != (low & 0x1FFFF)
            || (programmed_high >> 24) != lapic_id as u32 {
            return Err(ApicInitError::IoApicProgrammingFailed);
        }
        // Unmask only after the destination and masked vector were read back.
        ioapic_write(io_base, low_reg, low & !(1 << 16));
    }
    LOCAL_APIC_BASE.store(lapic as usize, Ordering::Release);
    APIC_ACTIVE.store(true, Ordering::Release);
    Ok(())
}

pub unsafe fn mask_legacy_pic() {
    unsafe {
        asm!("out dx, al", in("dx") PIC_MASTER_DATA, in("al") 0xFFu8, options(nomem, nostack));
        asm!("out dx, al", in("dx") PIC_SLAVE_DATA, in("al") 0xFFu8, options(nomem, nostack));
    }
}

pub fn apic_active() -> bool { APIC_ACTIVE.load(Ordering::Acquire) }

unsafe fn io_wait() {
    // SAFETY: Port 0x80 is the conventional legacy I/O delay port.
    unsafe { asm!("out 0x80, al", in("al") 0u8, options(nomem, nostack, preserves_flags)); }
}

/// Read the current legacy PIC interrupt masks.
pub fn legacy_pic_masks() -> (u8, u8) {
    unsafe {
        let master: u8;
        let slave: u8;
        asm!("in al, dx", in("dx") PIC_MASTER_DATA, out("al") master, options(nomem, nostack, preserves_flags));
        asm!("in al, dx", in("dx") PIC_SLAVE_DATA, out("al") slave, options(nomem, nostack, preserves_flags));
        (master, slave)
    }
}

/// Configure and remap both 8259 PICs, unmasking only IRQ0, then program PIT channel 0.
/// Interrupt delivery remains disabled until enable_hardware_interrupts is called.
pub unsafe fn initialize_pic_pit() {
    unsafe { asm!("cli", options(nomem, nostack)); }
    unsafe {
        asm!("out dx, al", in("dx") PIC_MASTER_COMMAND, in("al") 0x11u8, options(nomem, nostack));
        io_wait();
        asm!("out dx, al", in("dx") PIC_SLAVE_COMMAND, in("al") 0x11u8, options(nomem, nostack));
        io_wait();
        asm!("out dx, al", in("dx") PIC_MASTER_DATA, in("al") 0x20u8, options(nomem, nostack));
        io_wait();
        asm!("out dx, al", in("dx") PIC_SLAVE_DATA, in("al") 0x28u8, options(nomem, nostack));
        io_wait();
        // ICW3: slave is connected to master IRQ2; slave identity is 2.
        asm!("out dx, al", in("dx") PIC_MASTER_DATA, in("al") 0x04u8, options(nomem, nostack));
        io_wait();
        asm!("out dx, al", in("dx") PIC_SLAVE_DATA, in("al") 0x02u8, options(nomem, nostack));
        io_wait();
        // ICW4: 8086 mode.
        asm!("out dx, al", in("dx") PIC_MASTER_DATA, in("al") 0x01u8, options(nomem, nostack));
        io_wait();
        asm!("out dx, al", in("dx") PIC_SLAVE_DATA, in("al") 0x01u8, options(nomem, nostack));
        io_wait();
        // Mask every line except the PIT timer on IRQ0.
        asm!("out dx, al", in("dx") PIC_MASTER_DATA, in("al") 0xFEu8, options(nomem, nostack));
        io_wait();
        asm!("out dx, al", in("dx") PIC_SLAVE_DATA, in("al") 0xFFu8, options(nomem, nostack));
        io_wait();
        // PIT channel 0, low/high byte, mode 3 (square wave), binary counter.
        asm!("out dx, al", in("dx") PIT_COMMAND, in("al") 0x36u8, options(nomem, nostack));
        io_wait();
        asm!("out dx, al", in("dx") PIT_CHANNEL0, in("al") (PIT_DIVISOR as u8), options(nomem, nostack));
        io_wait();
        asm!("out dx, al", in("dx") PIT_CHANNEL0, in("al") ((PIT_DIVISOR >> 8) as u8), options(nomem, nostack));
        io_wait();
    }
    TIMER_TICKS.store(0, Ordering::Release);
}

/// Enable maskable hardware interrupts after the IDT, PIC and PIT are ready.
pub unsafe fn enable_hardware_interrupts() {
    unsafe { asm!("sti", options(nomem, nostack)); }
}

/// Disable maskable hardware interrupts.
pub unsafe fn disable_hardware_interrupts() {
    unsafe { asm!("cli", options(nomem, nostack)); }
}

/// Return the number of PIT timer ticks delivered since initialization.
pub fn timer_ticks() -> u64 {
    TIMER_TICKS.load(Ordering::Acquire)
}

/// Number of delivered hardware interrupts for a legacy IRQ line (0..15).
pub fn irq_count(irq: u8) -> Option<u64> {
    IRQ_COUNTS.get(irq as usize).map(|count| count.load(Ordering::Acquire))
}

/// Number of Local APIC spurious-vector deliveries (vector 0xFF).
pub fn apic_spurious_count() -> u64 {
    APIC_SPURIOUS_COUNT.load(Ordering::Acquire)
}

pub fn pic_spurious_counts() -> (u64, u64) {
    (PIC_SPURIOUS_7_COUNT.load(Ordering::Acquire), PIC_SPURIOUS_15_COUNT.load(Ordering::Acquire))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PicEoi {
    None,
    MasterOnly,
    SlaveThenMaster,
}

const fn pic_eoi_policy(irq: usize, in_service: bool) -> PicEoi {
    match irq {
        7 if !in_service => PicEoi::None,
        15 if !in_service => PicEoi::MasterOnly,
        8..=15 => PicEoi::SlaveThenMaster,
        _ => PicEoi::MasterOnly,
    }
}

unsafe fn read_pic_isr(command_port: u16) -> u8 {
    unsafe {
        asm!("out dx, al", in("dx") command_port, in("al") 0x0Bu8, options(nomem, nostack));
        let value: u8;
        asm!("in al, dx", in("dx") command_port, out("al") value, options(nomem, nostack));
        value
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn systemes_irq_dispatch(vector: u64) {
    // The Local APIC spurious vector is special: the CPU does not set ISR for it,
    // therefore an EOI must never be sent.
    if vector == APIC_SPURIOUS_VECTOR as u64 {
        APIC_SPURIOUS_COUNT.fetch_add(1, Ordering::Relaxed);
        return;
    }
    if !(IRQ_VECTOR_BASE as u64..(ACTIVE_VECTOR_COUNT as u64)).contains(&vector) {
        return;
    }

    let irq = vector as usize - IRQ_VECTOR_BASE;
    if irq == 0 {
        TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    }

    if APIC_ACTIVE.load(Ordering::Acquire) {
        IRQ_COUNTS[irq].fetch_add(1, Ordering::Relaxed);
        let base = LOCAL_APIC_BASE.load(Ordering::Acquire) as *mut u8;
        if !base.is_null() {
            // SAFETY: The UC-mapped Local APIC page is published before STI.
            unsafe { lapic_write(base, 0xB0, 0); }
        }
        return;
    }

    // 8259A IRQ7/IRQ15 may be spurious. Read the relevant In-Service Register
    // before deciding which PICs, if any, require an EOI.
    if irq == 7 || irq == 15 {
        let command = if irq == 7 { PIC_MASTER_COMMAND } else { PIC_SLAVE_COMMAND };
        let isr = unsafe { read_pic_isr(command) };
        let in_service = isr & (1 << 7) != 0;
        if !in_service && irq == 7 {
            PIC_SPURIOUS_7_COUNT.fetch_add(1, Ordering::Relaxed);
        } else if !in_service && irq == 15 {
            PIC_SPURIOUS_15_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        match pic_eoi_policy(irq, in_service) {
            PicEoi::None => {
                if irq == 7 { PIC_SPURIOUS_7_COUNT.fetch_add(1, Ordering::Relaxed); }
                return;
            },
            PicEoi::MasterOnly => {
                unsafe { asm!("out dx, al", in("dx") PIC_MASTER_COMMAND, in("al") 0x20u8, options(nomem, nostack)); }
                return;
            }
            PicEoi::SlaveThenMaster => {}
        }
    }
    IRQ_COUNTS[irq].fetch_add(1, Ordering::Relaxed);

    unsafe {
        if irq >= 8 {
            asm!("out dx, al", in("dx") PIC_SLAVE_COMMAND, in("al") 0x20u8, options(nomem, nostack));
        }
        asm!("out dx, al", in("dx") PIC_MASTER_COMMAND, in("al") 0x20u8, options(nomem, nostack));
    }
}

/// Validate all sixteen remapped legacy PIC IRQ gates before enabling interrupts.
pub fn verify_irq_gates() -> bool {
    let idt = addr_of!(IDT) as *const IdtEntry;
    for irq in 0..IRQ_COUNT {
        let entry = unsafe { &*idt.add(IRQ_VECTOR_BASE + irq) };
        let selector = unsafe { addr_of!(entry.selector).read_unaligned() };
        let attributes = unsafe { addr_of!(entry.attributes).read_unaligned() };
        let low = unsafe { addr_of!(entry.offset_low).read_unaligned() };
        let middle = unsafe { addr_of!(entry.offset_middle).read_unaligned() };
        let high = unsafe { addr_of!(entry.offset_high).read_unaligned() };
        let handler = low as u64 | ((middle as u64) << 16) | ((high as u64) << 32);
        if selector != KERNEL_CODE_SELECTOR
            || attributes != EXCEPTION_GATE_ATTRIBUTES
            || handler == 0
            || entry.ist != 0
        {
            return false;
        }
    }
    true
}

/// Install a private ring-0 GDT, TSS and IST-backed gates for CPU exceptions 0..31.
/// Hardware interrupts remain disabled.
pub unsafe fn initialize() {
    // SAFETY: This runs during single-core boot before the kernel enables IRQs.
    unsafe { asm!("cli", options(nomem, nostack)) };

    // Initialize independent, page-aligned emergency stacks before exposing IST gates.
    let rsp0_top = unsafe { addr_of_mut!(PRIVILEGE_STACK.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64 };
    let df_top = unsafe { addr_of_mut!(DOUBLE_FAULT_STACK.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64 };
    let nmi_top = unsafe { addr_of_mut!(NMI_STACK.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64 };
    let mc_top = unsafe { addr_of_mut!(MACHINE_CHECK_STACK.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64 };
    let tss = addr_of_mut!(TSS);
    unsafe {
        core::ptr::addr_of_mut!((*tss).rsp0).write_unaligned(rsp0_top);
        core::ptr::addr_of_mut!((*tss).ist1).write_unaligned(df_top);
        core::ptr::addr_of_mut!((*tss).ist2).write_unaligned(nmi_top);
        core::ptr::addr_of_mut!((*tss).ist3).write_unaligned(mc_top);
        core::ptr::addr_of_mut!((*tss).iomap_base).write_unaligned(core::mem::size_of::<TaskStateSegment>() as u16);
    }

    let tss_base = tss as u64;
    let [tss_low, tss_high] = encode_tss_descriptor(
        tss_base,
        (core::mem::size_of::<TaskStateSegment>() - 1) as u32,
    );
    let gdt = addr_of_mut!(GDT).cast::<u64>();
    unsafe {
        gdt.add(3).write(tss_low);
        gdt.add(4).write(tss_high);
    }
    let gdt_pointer = DescriptorTablePointer {
        limit: (core::mem::size_of::<[u64; 5]>() - 1) as u16,
        base: gdt as u64,
    };
    // SAFETY: GDT/TSS are static, identity-mapped, and remain alive for the kernel lifetime.
    unsafe { systemes_load_gdt(&gdt_pointer) };
    // SAFETY: The TSS descriptor at selector 0x18 is present and references the static TSS.
    unsafe { systemes_load_tr() };

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
        systemes_isr31 as *const () as usize
    ];
    let irq_handlers: [usize; IRQ_COUNT] = [
        systemes_irq0 as *const () as usize,
        systemes_irq1 as *const () as usize,
        systemes_irq2 as *const () as usize,
        systemes_irq3 as *const () as usize,
        systemes_irq4 as *const () as usize,
        systemes_irq5 as *const () as usize,
        systemes_irq6 as *const () as usize,
        systemes_irq7 as *const () as usize,
        systemes_irq8 as *const () as usize,
        systemes_irq9 as *const () as usize,
        systemes_irq10 as *const () as usize,
        systemes_irq11 as *const () as usize,
        systemes_irq12 as *const () as usize,
        systemes_irq13 as *const () as usize,
        systemes_irq14 as *const () as usize,
        systemes_irq15 as *const () as usize,
    ];
    let idt = addr_of_mut!(IDT) as *mut IdtEntry;
    for (vector, handler) in handlers.iter().enumerate() {
        let ist = match vector {
            2 => 2,  // NMI -> IST2
            8 => 1,  // Double Fault -> IST1
            18 => 3, // Machine Check -> IST3
            _ => 0,
        };
        // SAFETY: Each index is below EXCEPTION_VECTOR_COUNT and IDT has 256 entries.
        unsafe { idt.add(vector).write(IdtEntry::interrupt_gate(*handler as u64, ist)) };
    }

    for (irq, handler) in irq_handlers.iter().enumerate() {
        // SAFETY: IRQ vectors 32..47 are reserved by the PIC remap and IDT has 256 entries.
        unsafe { idt.add(IRQ_VECTOR_BASE + irq).write(IdtEntry::interrupt_gate(*handler as u64, 0)) };
    }

    // The Local APIC spurious vector is architecturally 0xFF; it must have a gate.
    unsafe { idt.add(0xFF).write(IdtEntry::interrupt_gate(systemes_apic_spurious as *const () as usize as u64, 0)) };

    let idt_pointer = DescriptorTablePointer {
        limit: (256 * core::mem::size_of::<IdtEntry>() - 1) as u16,
        base: idt as u64,
    };
    // SAFETY: IDT is static storage and all exception entries have been initialized.
    unsafe {
        asm!(
            "lidt [{}]",
            in(reg) &idt_pointer,
            options(readonly, nostack, preserves_flags)
        )
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn systemes_exception_dispatch(vector: u64, error_code: u64) -> ! {
    #[cfg(feature = "qemu-ist-nmi-test")]
    if vector == 2 {
        let rsp: u64;
        // SAFETY: The dispatcher runs at CPL0 after the CPU has applied the IDT IST2 switch.
        unsafe { asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags)); }
        let start = unsafe { addr_of!(NMI_STACK.0) as *const u8 as u64 };
        let end = start + INTERRUPT_STACK_SIZE as u64;
        unsafe {
            if rsp >= start && rsp < end {
                debug_write(b"SYSTEMES_TEST: IST2 NMI stack switch verified\r\n");
            } else {
                debug_write(b"SYSTEMES_KERNEL_FAIL: IST2 stack switch missing\r\n");
            }
        }
    }
    #[cfg(feature = "qemu-ist-machine-check-test")]
    if vector == 18 {
        let rsp: u64;
        // SAFETY: The dispatcher runs at CPL0 after the CPU has applied the IDT IST3 switch.
        unsafe { asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags)); }
        let start = unsafe { addr_of!(MACHINE_CHECK_STACK.0) as *const u8 as u64 };
        let end = start + INTERRUPT_STACK_SIZE as u64;
        unsafe {
            if rsp >= start && rsp < end {
                debug_write(b"SYSTEMES_TEST: IST3 machine-check stack switch verified\r\n");
            } else {
                debug_write(b"SYSTEMES_KERNEL_FAIL: IST3 stack switch missing\r\n");
            }
        }
    }
    unsafe {
        debug_write(b"SYSTEMES_EXCEPTION: vector=0x");
        debug_hex(vector);
        debug_write(b" error=0x");
        debug_hex(error_code);
        debug_write(b"\r\n");
        asm!("cli", options(nomem, nostack));
        loop {
            asm!("hlt", options(nomem, nostack));
        }
    }
}

unsafe fn outb(value: u8) {
    // SAFETY: Port 0xE9 is the QEMU debug console used by the boot workflow.
    unsafe { asm!("out dx, al", in("dx") 0xE9u16, in("al") value, options(nomem, nostack, preserves_flags)) };
}

unsafe fn debug_write(bytes: &[u8]) {
    for byte in bytes {
        unsafe { outb(*byte) };
    }
}

unsafe fn debug_hex(value: u64) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for shift in (0..16).rev() {
        unsafe { outb(HEX[((value >> (shift * 4)) & 0xF) as usize]) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_pic_vectors_follow_the_remapped_8259_layout() {
        assert_eq!(IRQ_VECTOR_BASE, 32);
        assert_eq!(IRQ_VECTOR_BASE + IRQ_COUNT - 1, 47);
    }

    #[test]
    fn pit_divisor_targets_approximately_one_hundred_hertz() {
        assert_eq!(PIT_DIVISOR, 11_931);
        let actual_hz = PIT_INPUT_HZ / PIT_DIVISOR as u32;
        assert!((95..=105).contains(&actual_hz));
    }

    #[test]
    fn encodes_a_64_bit_ring_zero_interrupt_gate() {
        let entry = IdtEntry::interrupt_gate(0x1234_5678_9ABC_DEF0, 3);
        let low = unsafe { addr_of!(entry.offset_low).read_unaligned() };
        let middle = unsafe { addr_of!(entry.offset_middle).read_unaligned() };
        let high = unsafe { addr_of!(entry.offset_high).read_unaligned() };
        assert_eq!(low, 0xDEF0);
        assert_eq!(middle, 0x9ABC);
        assert_eq!(high, 0x1234_5678);
        let selector = unsafe { addr_of!(entry.selector).read_unaligned() };
        let reserved = unsafe { addr_of!(entry.reserved).read_unaligned() };
        assert_eq!(selector, KERNEL_CODE_SELECTOR);
        assert_eq!(entry.ist, 3);
        assert_eq!(entry.attributes, EXCEPTION_GATE_ATTRIBUTES);
        assert_eq!(reserved, 0);
    }

    #[test]
    fn encodes_a_present_available_64_bit_tss_descriptor() {
        let [low, high] = encode_tss_descriptor(0x1234_5678_9ABC_DEF0, 103);
        assert_eq!(high, 0x1234_5678);
        assert_eq!(low & 0xFFFF, 103);
        assert_eq!((low >> 40) & 0xFF, 0x89);
        assert_eq!((low >> 56) & 0xFF, 0x9A);
    }

    #[test]
    fn tss_layout_matches_the_64_bit_architectural_format() {
        assert_eq!(core::mem::size_of::<TaskStateSegment>(), 104);
        assert_eq!(core::mem::offset_of!(TaskStateSegment, rsp0), 4);
        assert_eq!(core::mem::offset_of!(TaskStateSegment, ist1), 36);
        assert_eq!(core::mem::offset_of!(TaskStateSegment, ist2), 44);
        assert_eq!(core::mem::offset_of!(TaskStateSegment, ist3), 52);
        assert_eq!(core::mem::offset_of!(TaskStateSegment, iomap_base), 102);
    }

    #[test]
    fn critical_exception_vectors_use_distinct_ist_slots() {
        let double_fault = IdtEntry::interrupt_gate(0x1000, 1);
        let nmi = IdtEntry::interrupt_gate(0x2000, 2);
        let machine_check = IdtEntry::interrupt_gate(0x3000, 3);
        assert_eq!((double_fault.ist, nmi.ist, machine_check.ist), (1, 2, 3));
    }

    #[test]
    fn pic_spurious_irq7_sends_no_eoi() {
        assert_eq!(pic_eoi_policy(7, false), PicEoi::None);
        assert_eq!(pic_eoi_policy(7, true), PicEoi::MasterOnly);
    }

    #[test]
    fn pic_spurious_irq15_sends_master_eoi_only() {
        assert_eq!(pic_eoi_policy(15, false), PicEoi::MasterOnly);
        assert_eq!(pic_eoi_policy(15, true), PicEoi::SlaveThenMaster);
    }

    #[test]
    fn ordinary_pic_irqs_keep_their_expected_eoi_path() {
        assert_eq!(pic_eoi_policy(0, true), PicEoi::MasterOnly);
        assert_eq!(pic_eoi_policy(1, true), PicEoi::MasterOnly);
        assert_eq!(pic_eoi_policy(12, true), PicEoi::SlaveThenMaster);
    }

    #[test]
    fn identifies_cpu_exceptions_that_push_an_error_code() {
        for vector in [8, 10, 11, 12, 13, 14, 17, 21, 29, 30] {
            assert!(has_cpu_error_code(vector));
        }
        for vector in [0, 1, 2, 3, 4, 5, 6, 7, 9, 15, 16, 18, 19, 20, 31] {
            assert!(!has_cpu_error_code(vector));
        }
    }
}

#[cfg(test)]
const fn has_cpu_error_code(vector: u8) -> bool {
    matches!(vector, 8 | 10 | 11 | 12 | 13 | 14 | 17 | 21 | 29 | 30)
}
