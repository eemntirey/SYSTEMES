//! x86_64 descriptor tables and fatal CPU-exception diagnostics.
//!
//! The kernel remains in the UEFI-provided long-mode environment. This module
//! installs its own GDT and IDT, then dispatches the Local APIC timer and task-yield vector.
//! It also configures SYSCALL/SYSRET for fast Ring 3 transitions.

use core::arch::{asm, global_asm};
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::console::{debug_hex, debug_write};

const KERNEL_CODE_SELECTOR: u16 = 0x08;
const KERNEL_DATA_SELECTOR: u16 = 0x10;
const TSS_SELECTOR: u16 = 0x18;
const USER_DATA_SELECTOR: u16 = 0x28;
const USER_CODE_SELECTOR: u16 = 0x30;
const USER_DATA_RPL3: u16 = USER_DATA_SELECTOR | 3;
const USER_CODE_RPL3: u16 = USER_CODE_SELECTOR | 3;
const EXCEPTION_GATE_ATTRIBUTES: u8 = 0x8E;
const USER_INTERRUPT_GATE_ATTRIBUTES: u8 = 0xEE;
const USER_DATA_DESCRIPTOR: u64 = 0x00CF_F200_0000_FFFF;
const USER_CODE_DESCRIPTOR: u64 = 0x00AF_FA00_0000_FFFF;
const EXCEPTION_VECTOR_COUNT: usize = 32;
const INTERRUPT_STACK_SIZE: usize = 16 * 1024;
const USER_EXIT_VECTOR: u64 = 0x80;

pub(crate) const RING3_TEST_MAGIC: u64 = 0x5233_5F54_4553_5421;

/// Vector raised by the INT3 instruction.
pub const BREAKPOINT_VECTOR: u64 = 3;

static mut GDT: [u64; 7] = [
    0,
    0x00AF_9A00_0000_FFFF,
    0x00CF_9200_0000_FFFF,
    0,
    0,
    USER_DATA_DESCRIPTOR,
    USER_CODE_DESCRIPTOR,
];

#[repr(C, packed)]
struct TaskStateSegment {
    reserved0: u32, rsp0: u64, rsp1: u64, rsp2: u64, reserved1: u64,
    ist1: u64, ist2: u64, ist3: u64, ist4: u64, ist5: u64, ist6: u64, ist7: u64,
    reserved2: u64, reserved3: u16, iomap_base: u16,
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
#[unsafe(no_mangle)]
static mut SYSTEMES_SYSCALL_KERNEL_STACK: InterruptStack =
    InterruptStack([0; INTERRUPT_STACK_SIZE]);
#[unsafe(no_mangle)]
static mut SYSTEMES_SYSCALL_USER_RSP: u64 = 0;
#[unsafe(no_mangle)]
static SYSTEMES_SYSCALL_EXIT_STATUS: AtomicU64 = AtomicU64::new(0);
static RING3_ACTIVE: AtomicBool = AtomicBool::new(false);
static RING3_EXIT_VALIDATED: AtomicBool = AtomicBool::new(false);
static RING3_EXPECTED_USER_RSP: AtomicU64 = AtomicU64::new(0);

#[unsafe(no_mangle)]
static mut SYSTEMES_RING3_SAVED_KERNEL_RSP: u64 = 0;

const fn encode_tss_descriptor(base: u64, limit: u32) -> [u64; 2] {
    let low = (limit as u64 & 0xFFFF)
        | ((base & 0x00FF_FFFF) << 16)
        | (0x89u64 << 40)
        | ((((limit as u64) >> 16) & 0x0F) << 48)
        | (((base >> 24) & 0xFF) << 56);
    [low, base >> 32]
}

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

/// Register frame saved by the exception stubs for same-privilege (ring 0)
/// exceptions. The CPU pushes RIP/CS/RFLAGS; the stub adds the vector and a
/// synthetic error code so every vector shares one layout.
#[repr(C)]
pub struct ExceptionFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub vector: u64,
    pub error_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
}

/// Return true only for the one exception vector the handler resumes.
///
/// `INT3` is a trap: the CPU saves the address of the instruction following
/// the opcode, so resuming requires restoring the frame and executing `iretq`
/// without modifying RIP.
pub const fn is_resumable_exception(vector: u64) -> bool {
    vector == BREAKPOINT_VECTOR
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::MISSING; 256];

unsafe extern "C" {
    fn systemes_load_gdt(pointer: *const DescriptorTablePointer);
    fn systemes_load_tr();
    fn systemes_enter_ring3(user_rip: u64, user_rsp: u64);
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

global_asm!(r#"
.global systemes_load_gdt
.type systemes_load_gdt, @function
systemes_load_gdt:
    lgdt [rdi]
    push {kernel_code_selector}
    lea rax, [rip + .Lsystemes_reload_cs]
    push rax
    retfq
.Lsystemes_reload_cs:
    mov ax, {kernel_data_selector}
    mov ds, ax
    mov es, ax
    mov ss, ax
    ret

.global systemes_enter_ring3
.type systemes_enter_ring3, @function
systemes_enter_ring3:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov qword ptr [rip + SYSTEMES_RING3_SAVED_KERNEL_RSP], rsp
    push {user_data_selector}
    push rsi
    push 0x02
    push {user_code_selector}
    push rdi
    iretq

.global systemes_ring3_exit_trampoline
.type systemes_ring3_exit_trampoline, @function
systemes_ring3_exit_trampoline:
    mov ax, {kernel_data_selector}
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov rsp, qword ptr [rip + SYSTEMES_RING3_SAVED_KERNEL_RSP]
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret

.global systemes_load_tr
.type systemes_load_tr, @function
systemes_load_tr:
    mov ax, {tss_selector}
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

.global systemes_user_exit
.type systemes_user_exit, @function
systemes_user_exit:
    push 0
    push 128
    jmp systemes_exception_common

.global systemes_syscall_entry
.type systemes_syscall_entry, @function
systemes_syscall_entry:
    mov qword ptr [rip + SYSTEMES_SYSCALL_USER_RSP], rsp
    lea rsp, [rip + SYSTEMES_SYSCALL_KERNEL_STACK]
    add rsp, {interrupt_stack_size}
    push qword ptr [rip + SYSTEMES_SYSCALL_USER_RSP]
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
    mov rdi, rsp
    call systemes_syscall_dispatch
    cmp rax, -1
    je .Lsystemes_syscall_exit
    mov qword ptr [rsp + 112], rax
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
    add rsp, 8
    mov rsp, qword ptr [rip + SYSTEMES_SYSCALL_USER_RSP]
    sysretq
.Lsystemes_syscall_exit:
    mov rax, qword ptr [rip + SYSTEMES_SYSCALL_EXIT_STATUS]
    jmp systemes_ring3_exit_trampoline

.global systemes_lapic_timer
.type systemes_lapic_timer, @function
systemes_lapic_timer:
    push 0
    push 64
    jmp systemes_irq_common

.global systemes_scheduler_yield
.type systemes_scheduler_yield, @function
systemes_scheduler_yield:
    push 0
    push 65
    jmp systemes_irq_common

.global systemes_apic_spurious
.type systemes_apic_spurious, @function
systemes_apic_spurious:
    iretq

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
    mov r12, rsp
    and rsp, -16
    call systemes_fpu_save_current
    mov rsp, r12
    mov rdi, qword ptr [rsp + 120]
    mov rsi, rsp
    and rsp, -16
    call systemes_apic_timer_dispatch
    mov r12, rax
    and rsp, -16
    call systemes_fpu_restore_current
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
    mov r12, rsp
    mov rdi, qword ptr [rsp + 120]
    mov rsi, qword ptr [rsp + 128]
    mov rdx, r12
    and rsp, -16
    call systemes_exception_dispatch
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
"#,
    kernel_code_selector = const KERNEL_CODE_SELECTOR,
    kernel_data_selector = const KERNEL_DATA_SELECTOR,
    tss_selector = const TSS_SELECTOR,
    user_data_selector = const USER_DATA_RPL3,
    user_code_selector = const USER_CODE_RPL3,
    interrupt_stack_size = const INTERRUPT_STACK_SIZE,
);

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

/// Verify the Local APIC timer, cooperative-yield and spurious-vector IDT gates.
pub fn verify_local_apic_gates() -> bool {
    let idt = addr_of!(IDT) as *const IdtEntry;
    let timer = unsafe { &*idt.add(0x40) };
    let yield_gate = unsafe { &*idt.add(0x41) };
    let spurious = unsafe { &*idt.add(0xFF) };
    let timer_selector = unsafe { addr_of!(timer.selector).read_unaligned() };
    let timer_attributes = unsafe { addr_of!(timer.attributes).read_unaligned() };
    let timer_low = unsafe { addr_of!(timer.offset_low).read_unaligned() };
    let timer_middle = unsafe { addr_of!(timer.offset_middle).read_unaligned() };
    let timer_high = unsafe { addr_of!(timer.offset_high).read_unaligned() };
    let timer_handler = timer_low as u64 | ((timer_middle as u64) << 16) | ((timer_high as u64) << 32);
    let yield_selector = unsafe { addr_of!(yield_gate.selector).read_unaligned() };
    let yield_attributes = unsafe { addr_of!(yield_gate.attributes).read_unaligned() };
    let yield_low = unsafe { addr_of!(yield_gate.offset_low).read_unaligned() };
    let yield_middle = unsafe { addr_of!(yield_gate.offset_middle).read_unaligned() };
    let yield_high = unsafe { addr_of!(yield_gate.offset_high).read_unaligned() };
    let yield_handler = yield_low as u64 | ((yield_middle as u64) << 16) | ((yield_high as u64) << 32);
    let spurious_selector = unsafe { addr_of!(spurious.selector).read_unaligned() };
    let spurious_attributes = unsafe { addr_of!(spurious.attributes).read_unaligned() };
    let spurious_low = unsafe { addr_of!(spurious.offset_low).read_unaligned() };
    let spurious_middle = unsafe { addr_of!(spurious.offset_middle).read_unaligned() };
    let spurious_high = unsafe { addr_of!(spurious.offset_high).read_unaligned() };
    let spurious_handler = spurious_low as u64 | ((spurious_middle as u64) << 16) | ((spurious_high as u64) << 32);
    timer_selector == KERNEL_CODE_SELECTOR
        && timer_attributes == EXCEPTION_GATE_ATTRIBUTES
        && timer_handler != 0
        && yield_selector == KERNEL_CODE_SELECTOR
        && yield_attributes == EXCEPTION_GATE_ATTRIBUTES
        && yield_handler != 0
        && spurious_selector == KERNEL_CODE_SELECTOR
        && spurious_attributes == EXCEPTION_GATE_ATTRIBUTES
        && spurious_handler != 0
}

/// Verify that the user descriptors and the DPL3 exit gate are installed.
pub fn verify_ring3_configuration() -> bool {
    let gdt = addr_of!(GDT).cast::<u64>();
    let idt = addr_of!(IDT) as *const IdtEntry;
    // SAFETY: GDT entries 5/6 are the user data/code descriptors.
    let user_data = unsafe { gdt.add(5).read() };
    let user_code = unsafe { gdt.add(6).read() };
    // SAFETY: IDT vector 0x80 is initialized during interrupt setup.
    let gate = unsafe { &*idt.add(USER_EXIT_VECTOR as usize) };
    let selector = unsafe { addr_of!(gate.selector).read_unaligned() };
    let attributes = unsafe { addr_of!(gate.attributes).read_unaligned() };
    let low = unsafe { addr_of!(gate.offset_low).read_unaligned() };
    let middle = unsafe { addr_of!(gate.offset_middle).read_unaligned() };
    let high = unsafe { addr_of!(gate.offset_high).read_unaligned() };
    let handler = low as u64 | ((middle as u64) << 16) | ((high as u64) << 32);

    user_data == USER_DATA_DESCRIPTOR
        && user_code == USER_CODE_DESCRIPTOR
        && selector == KERNEL_CODE_SELECTOR
        && attributes == USER_INTERRUPT_GATE_ATTRIBUTES
        && handler != 0
}

const MSR_EFER: u32 = 0xC000_0080;
const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_SFMASK: u32 = 0xC000_0084;
const EFER_SCE: u64 = 1;
const STAR_VALUE: u64 =
    (((USER_DATA_RPL3 - 8) as u64) << 48) | ((KERNEL_CODE_SELECTOR as u64) << 32);
const SFMASK_VALUE: u64 =
    (1 << 8) | (1 << 9) | (1 << 10) | (3 << 12) | (1 << 14) | (1 << 16) | (1 << 18);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyscallInitError {
    UnsupportedByCpu,
}

/// Enable long-mode syscall entry and program the SYSCALL/SYSRET selectors.
///
/// # Safety
///
/// Must run at CPL0 after the kernel GDT is active and before user code can
/// execute `SYSCALL`. This kernel currently uses one shared syscall stack and
/// therefore supports only one CPU.
pub unsafe fn initialize_syscalls() -> Result<(), SyscallInitError> {
    let maximum_extended_leaf = core::arch::x86_64::__cpuid(0x8000_0000).eax;
    if maximum_extended_leaf < 0x8000_0001
        || core::arch::x86_64::__cpuid(0x8000_0001).edx & (1 << 11) == 0
    {
        return Err(SyscallInitError::UnsupportedByCpu);
    }

    write_msr(MSR_STAR, STAR_VALUE);
    write_msr(
        MSR_LSTAR,
        systemes_syscall_entry as *const () as usize as u64,
    );
    write_msr(MSR_SFMASK, SFMASK_VALUE);
    let efer = read_msr(MSR_EFER);
    write_msr(MSR_EFER, efer | EFER_SCE);
    Ok(())
}

/// Confirm the enabled syscall bit and programmed selectors/entry mask.
pub fn verify_syscall_configuration() -> bool {
    let stack_start = addr_of!(SYSTEMES_SYSCALL_KERNEL_STACK) as *const u8 as usize;
    let stack_top = stack_start + core::mem::size_of::<InterruptStack>();
    read_msr(MSR_EFER) & EFER_SCE != 0
        && read_msr(MSR_STAR) == STAR_VALUE
        && read_msr(MSR_LSTAR) == systemes_syscall_entry as *const () as usize as u64
        && read_msr(MSR_SFMASK) == SFMASK_VALUE
        && stack_start != 0
        && stack_top & 0x0F == 0
}

fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: Callers pass architectural MSRs while running at CPL0.
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack)
        )
    };
    ((high as u64) << 32) | low as u64
}

fn write_msr(msr: u32, value: u64) {
    // SAFETY: Callers pass architectural MSRs while running at CPL0.
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nomem, nostack)
        )
    };
}

/// Enter a mapped ring-3 test context and return through its DPL3 interrupt.
///
/// # Safety
///
/// The caller must have installed the kernel GDT/IDT/TSS, mapped `user_rip`
/// executable and `user_rsp` writable in the active address space, and disabled
/// interrupts. The user code must use a supported exit path: `int 0x80` or `SYS_EXIT`.
#[cfg(feature = "qemu-ring3-test")]
pub(crate) unsafe fn enter_ring3_test_context(user_rip: u64, user_rsp: u64) -> bool {
    RING3_EXIT_VALIDATED.store(false, Ordering::Release);
    RING3_EXPECTED_USER_RSP.store(user_rsp, Ordering::Release);
    RING3_ACTIVE.store(true, Ordering::Release);
    SYSTEMES_SYSCALL_EXIT_STATUS.store(0, Ordering::Release);
    // SAFETY: The caller guarantees valid user mappings and a configured TSS.
    unsafe { systemes_enter_ring3(user_rip, user_rsp) };
    RING3_ACTIVE.store(false, Ordering::Release);
    RING3_EXIT_VALIDATED.load(Ordering::Acquire)
}

/// Enter ring 3 at `rip` with `rsp` as the user stack. Returns the status passed
/// to `SYS_EXIT`; a user process that does not exit or faults remains non-returning.
///
/// # Safety
///
/// The caller must have installed the GDT/IDT/TSS, mapped `rip` executable
/// and `rsp` writable in the active address space, and disabled interrupts.
pub unsafe fn enter_user_mode(rip: u64, rsp: u64) -> u64 {
    SYSTEMES_SYSCALL_EXIT_STATUS.store(0, Ordering::Release);
    // SAFETY: The caller guarantees valid user mappings and a configured TSS.
    unsafe { systemes_enter_ring3(rip, rsp) };
    user_exit_status()
}

pub(crate) fn set_user_exit_status(status: u64) {
    SYSTEMES_SYSCALL_EXIT_STATUS.store(status, Ordering::Release);
}

pub(crate) fn user_exit_status() -> u64 {
    SYSTEMES_SYSCALL_EXIT_STATUS.load(Ordering::Acquire)
}

/// Install a private ring-0 GDT and gates for all architecturally defined
/// exception vectors 0..31. Hardware interrupts remain disabled.
pub unsafe fn initialize() {
    // SAFETY: This runs during single-core boot before the kernel enables IRQs.
    unsafe { asm!("cli", options(nomem, nostack)) };

    // Initialize independent emergency stacks before exposing IST-backed gates.
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
    let [tss_low, tss_high] = encode_tss_descriptor(tss_base, (core::mem::size_of::<TaskStateSegment>() - 1) as u32);
    let gdt = addr_of_mut!(GDT).cast::<u64>();
    unsafe {
        gdt.add(3).write(tss_low);
        gdt.add(4).write(tss_high);
    }
    let gdt_pointer = DescriptorTablePointer {
        limit: (core::mem::size_of::<[u64; 7]>() - 1) as u16,
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
    let idt = addr_of_mut!(IDT) as *mut IdtEntry;
    for (vector, handler) in handlers.iter().enumerate() {
        let ist = match vector { 2 => 2, 8 => 1, 18 => 3, _ => 0 };
        // SAFETY: Each index is below EXCEPTION_VECTOR_COUNT and IDT has 256 entries.
        unsafe { idt.add(vector).write(IdtEntry::interrupt_gate(*handler as u64, ist)) };
    }

    // Local APIC timer and spurious vectors are installed above the exception range.
    unsafe {
        idt.add(0x40).write(IdtEntry::interrupt_gate(systemes_lapic_timer as *const () as usize as u64, 0));
        idt.add(0x41).write(IdtEntry::interrupt_gate(systemes_scheduler_yield as *const () as usize as u64, 0));
        idt.add(0xFF).write(IdtEntry::interrupt_gate(systemes_apic_spurious as *const () as usize as u64, 0));
        idt.add(USER_EXIT_VECTOR as usize).write(IdtEntry::user_interrupt_gate(systemes_user_exit as *const () as usize as u64));
    }

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

/// Dispatch CPU exceptions. INT3 resumes unchanged; the dedicated Ring 3 test
/// exit rewrites its frame to a controlled kernel trampoline. Other exceptions
/// remain fatal.
#[unsafe(no_mangle)]
pub extern "C" fn systemes_exception_dispatch(
    vector: u64,
    error_code: u64,
    frame: *mut ExceptionFrame,
) {
    #[cfg(feature = "qemu-ist-nmi-test")]
    if vector == 2 {
        let rsp: u64;
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

    if vector == USER_EXIT_VECTOR && RING3_ACTIVE.load(Ordering::Acquire) {
        if let Some(frame_ref) = unsafe { frame.as_mut() } {
            let privilege_frame = frame.cast::<u64>();
            // A CPL3 interrupt frame appends the interrupted RSP and SS after RFLAGS.
            let user_rsp = unsafe { privilege_frame.add(20).read() };
            let user_ss = unsafe { privilege_frame.add(21).read() };
            if error_code == 0
                && valid_ring3_exit_frame(
                    frame_ref,
                    user_rsp,
                    user_ss,
                    RING3_EXPECTED_USER_RSP.load(Ordering::Acquire),
                )
            {
                frame_ref.rip = systemes_ring3_exit_trampoline as *const () as usize as u64;
                frame_ref.cs = KERNEL_CODE_SELECTOR as u64;
                frame_ref.rflags = 0x02;
                let kernel_rsp = unsafe {
                    addr_of!(SYSTEMES_RING3_SAVED_KERNEL_RSP).read_volatile()
                };
                // Rewrite the privilege-return stack slots along with CS.
                unsafe {
                    privilege_frame.add(20).write(kernel_rsp);
                    privilege_frame
                        .add(21)
                        .write(KERNEL_DATA_SELECTOR as u64);
                }
                RING3_ACTIVE.store(false, Ordering::Release);
                RING3_EXIT_VALIDATED.store(true, Ordering::Release);
                unsafe {
                    debug_write(b"SYSTEMES_TEST: Ring 3 interrupt returned through TSS RSP0\r\n");
                }
                return;
            }
        }
        unsafe {
            debug_write(b"SYSTEMES_KERNEL_FAIL: invalid Ring 3 exit frame\r\n");
        }
    }

    unsafe {
        debug_write(b"SYSTEMES_EXCEPTION: vector=0x");
        debug_hex(vector);
        debug_write(b" error=0x");
        debug_hex(error_code);
        debug_write(b"\r\n");
    }

    if is_resumable_exception(vector) {
        // The frame is not modified: INT3 is a trap, so the saved RIP already
        // points to the instruction following the 0xCC opcode.
        unsafe { debug_write(b"SYSTEMES: breakpoint handled, execution resumed\r\n"); }
        return;
    }

    unsafe {
        asm!("cli", options(nomem, nostack));
        loop {
            asm!("hlt", options(nomem, nostack));
        }
    }
}

fn valid_ring3_exit_frame(
    frame: &ExceptionFrame,
    user_rsp: u64,
    user_ss: u64,
    expected_user_rsp: u64,
) -> bool {
    frame.rax == RING3_TEST_MAGIC
        && frame.cs == USER_CODE_RPL3 as u64
        && user_ss == USER_DATA_RPL3 as u64
        && user_rsp == expected_user_rsp
        && frame.rflags & 0x2 != 0
        && frame.rflags & (1 << 9) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let df = IdtEntry::interrupt_gate(0x1000, 1);
        let nmi = IdtEntry::interrupt_gate(0x2000, 2);
        let mc = IdtEntry::interrupt_gate(0x3000, 3);
        assert_eq!((df.ist, nmi.ist, mc.ist), (1, 2, 3));
    }

    #[test]
    fn ring3_exit_gate_is_dpl3_and_uses_kernel_code() {
        let entry = IdtEntry::user_interrupt_gate(0x1234_5678_9ABC_DEF0);
        let selector = unsafe { addr_of!(entry.selector).read_unaligned() };
        assert_eq!(selector, KERNEL_CODE_SELECTOR);
        assert_eq!(entry.attributes, USER_INTERRUPT_GATE_ATTRIBUTES);
        assert_eq!(entry.ist, 0);
    }

    #[test]
    fn ring3_segments_have_dpl3_and_expected_code_data_types() {
        assert_eq!((USER_CODE_DESCRIPTOR >> 45) & 3, 3);
        assert_eq!((USER_DATA_DESCRIPTOR >> 45) & 3, 3);
        assert_ne!(USER_CODE_DESCRIPTOR & (1 << 43), 0);
        assert_ne!(USER_DATA_DESCRIPTOR & (1 << 41), 0);
        assert_eq!(USER_CODE_RPL3 & 3, 3);
        assert_eq!(USER_DATA_RPL3 & 3, 3);
    }

    #[test]
    fn syscall_star_selectors_match_the_gdt_layout() {
        let syscall_code = (STAR_VALUE >> 32) as u16;
        let sysret_base = (STAR_VALUE >> 48) as u16;
        assert_eq!(syscall_code, KERNEL_CODE_SELECTOR);
        assert_eq!(syscall_code + 8, KERNEL_DATA_SELECTOR);
        assert_eq!(sysret_base + 8, USER_DATA_RPL3);
        assert_eq!(sysret_base + 16, USER_CODE_RPL3);
        assert_eq!(SFMASK_VALUE & ((1 << 8) | (1 << 9) | (1 << 10)), (1 << 8) | (1 << 9) | (1 << 10));
    }

    #[test]
    fn ring3_exit_frame_requires_user_selectors_expected_stack_and_magic() {
        let mut frame = ExceptionFrame {
            r15: 0, r14: 0, r13: 0, r12: 0, r11: 0, r10: 0, r9: 0, r8: 0,
            rbp: 0, rdi: 0, rsi: 0, rdx: 0, rcx: 0, rbx: 0,
            rax: RING3_TEST_MAGIC, vector: USER_EXIT_VECTOR, error_code: 0,
            rip: 0x8000_0000_0011, cs: USER_CODE_RPL3 as u64, rflags: 0x02,
        };
        assert!(valid_ring3_exit_frame(&frame, 0x8000_0000_2FF0, USER_DATA_RPL3 as u64, 0x8000_0000_2FF0));

        frame.cs = KERNEL_CODE_SELECTOR as u64;
        assert!(!valid_ring3_exit_frame(&frame, 0x8000_0000_2FF0, USER_DATA_RPL3 as u64, 0x8000_0000_2FF0));
        frame.cs = USER_CODE_RPL3 as u64;
        frame.rflags |= 1 << 9;
        assert!(!valid_ring3_exit_frame(&frame, 0x8000_0000_2FF0, USER_DATA_RPL3 as u64, 0x8000_0000_2FF0));
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

    #[test]
    fn resumes_only_the_breakpoint_vector() {
        assert_eq!(BREAKPOINT_VECTOR, 3);
        assert!(is_resumable_exception(BREAKPOINT_VECTOR));
        for vector in [0, 1, 2, 6, 8, 13, 14, 18, 31] {
            assert!(!is_resumable_exception(vector));
        }
    }

    #[test]
    fn exception_frame_matches_the_assembly_layout() {
        assert_eq!(core::mem::size_of::<ExceptionFrame>(), 20 * 8);
        assert_eq!(core::mem::offset_of!(ExceptionFrame, r15), 0);
        assert_eq!(core::mem::offset_of!(ExceptionFrame, rax), 112);
        assert_eq!(core::mem::offset_of!(ExceptionFrame, vector), 120);
        assert_eq!(core::mem::offset_of!(ExceptionFrame, error_code), 128);
        assert_eq!(core::mem::offset_of!(ExceptionFrame, rip), 136);
        assert_eq!(core::mem::offset_of!(ExceptionFrame, cs), 144);
        assert_eq!(core::mem::offset_of!(ExceptionFrame, rflags), 152);
    }
}

#[cfg(test)]
const fn has_cpu_error_code(vector: u8) -> bool {
    matches!(vector, 8 | 10 | 11 | 12 | 13 | 14 | 17 | 21 | 29 | 30)
}
