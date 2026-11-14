//! x86_64 descriptor tables and fatal CPU-exception diagnostics.
//!
//! The kernel remains in the UEFI-provided long-mode environment. This module
//! installs its own GDT and IDT, then dispatches the Local APIC timer and task-yield vector.
//! It also configures SYSCALL/SYSRET for fast Ring 3 transitions.
//!
//! P1-6b: Per-CPU GDT/IDT/TSS support. Each CPU has its own descriptor tables and
//! interrupt stacks. The bootstrap CPU initializes its own tables first, then each
//! application processor initializes its own tables after SMP bring-up.

use core::arch::{asm, global_asm};
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

use crate::console::{debug_hex, debug_write};

const KERNEL_CODE_SELECTOR: u16 = 0x08;
const KERNEL_DATA_SELECTOR: u16 = 0x10;
const TSS_SELECTOR: u16 = 0x18;
const USER_DATA_SELECTOR: u16 = 0x28;
const USER_CODE_SELECTOR: u16 = 0x30;
const USER_DATA_RPL3: u16 = USER_DATA_SELECTOR | 3;
const USER_CODE_RPL3: u16 = USER_CODE_SELECTOR | 3;
const USER_MODE_RFLAGS: u64 = 0x202;
#[allow(dead_code)]
const USER_TEST_RFLAGS: u64 = 0x02;
const EXCEPTION_GATE_ATTRIBUTES: u8 = 0x8E;
const USER_INTERRUPT_GATE_ATTRIBUTES: u8 = 0xEE;
const USER_DATA_DESCRIPTOR: u64 = 0x00CF_F200_0000_FFFF;
const USER_CODE_DESCRIPTOR: u64 = 0x00AF_FA00_0000_FFFF;
const EXCEPTION_VECTOR_COUNT: usize = 32;
const INTERRUPT_STACK_SIZE: usize = 16 * 1024;
const USER_EXIT_VECTOR: u64 = 0x80;
const USER_FAULT_EXIT_STATUS: u64 = 0xF001_FA017_FA017001;

/// Maximum number of CPUs supported (matches smp::MAX_CPUS_PER_APIC_ID).
pub const MAX_CPUS: usize = 64;

pub(crate) const RING3_TEST_MAGIC: u64 = 0x5233_5F54_4553_5421;

/// Vector raised by the INT3 instruction.
pub const BREAKPOINT_VECTOR: u64 = 3;

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

/// Per-CPU syscall and Ring 3 transition state. GS_BASE points here in Ring 0.
#[repr(C, align(16))]
struct SyscallCpuLocal {
    syscall_stack: [u8; INTERRUPT_STACK_SIZE],
    syscall_stack_top: u64,
    user_rsp: AtomicU64,
    saved_kernel_rsp: AtomicU64,
    ring3_active: AtomicBool,
    exit_validated: AtomicBool,
    expected_user_rsp: AtomicU64,
    exit_status: AtomicU64,
    fault_vector: AtomicU64,
    fault_error: AtomicU64,
    fault_cr2: AtomicU64,
    fault_rip: AtomicU64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserFaultInfo {
    pub vector: u64,
    pub error_code: u64,
    pub cr2: u64,
    pub rip: u64,
}

impl SyscallCpuLocal {
    const fn new() -> Self {
        Self {
            syscall_stack: [0; INTERRUPT_STACK_SIZE],
            syscall_stack_top: 0,
            user_rsp: AtomicU64::new(0),
            saved_kernel_rsp: AtomicU64::new(0),
            ring3_active: AtomicBool::new(false),
            exit_validated: AtomicBool::new(false),
            expected_user_rsp: AtomicU64::new(0),
            exit_status: AtomicU64::new(0),
            fault_vector: AtomicU64::new(0),
            fault_error: AtomicU64::new(0),
            fault_cr2: AtomicU64::new(0),
            fault_rip: AtomicU64::new(0),
        }
    }
}

/// Per-CPU context containing the GS anchor, descriptor tables and interrupt stacks.
#[repr(C, align(4096))]
struct CpuContext {
    syscall: SyscallCpuLocal,
    gdt: [u64; 7],
    tss: TaskStateSegment,
    idt: [IdtEntry; 256],
    privilege_stack: InterruptStack,
    double_fault_stack: InterruptStack,
    nmi_stack: InterruptStack,
    machine_check_stack: InterruptStack,
}

impl CpuContext {
    const fn new() -> Self {
        Self {
            syscall: SyscallCpuLocal::new(),
            gdt: [0; 7],
            tss: TaskStateSegment {
                reserved0: 0, rsp0: 0, rsp1: 0, rsp2: 0, reserved1: 0,
                ist1: 0, ist2: 0, ist3: 0, ist4: 0, ist5: 0, ist6: 0, ist7: 0,
                reserved2: 0, reserved3: 0,
                iomap_base: core::mem::size_of::<TaskStateSegment>() as u16,
            },
            idt: [IdtEntry::MISSING; 256],
            privilege_stack: InterruptStack([0; INTERRUPT_STACK_SIZE]),
            double_fault_stack: InterruptStack([0; INTERRUPT_STACK_SIZE]),
            nmi_stack: InterruptStack([0; INTERRUPT_STACK_SIZE]),
            machine_check_stack: InterruptStack([0; INTERRUPT_STACK_SIZE]),
        }
    }
}

/// Per-CPU contexts, indexed by CPU slot (APIC ID).
static mut CPU_CONTEXTS: [CpuContext; MAX_CPUS] = [const { CpuContext::new() }; MAX_CPUS];

/// Track which CPU contexts have been initialized.
static CPU_INIT_STATE: [AtomicU8; MAX_CPUS] = [const { AtomicU8::new(0) }; MAX_CPUS];

const MSR_GS_BASE: u32 = 0xC000_0101;
const MSR_KERNEL_GS_BASE: u32 = 0xC000_0102;
const GS_SYSCALL_STACK_TOP: usize = core::mem::offset_of!(SyscallCpuLocal, syscall_stack_top);
const GS_USER_RSP: usize = core::mem::offset_of!(SyscallCpuLocal, user_rsp);
const GS_SAVED_KERNEL_RSP: usize = core::mem::offset_of!(SyscallCpuLocal, saved_kernel_rsp);
const GS_EXIT_STATUS: usize = core::mem::offset_of!(SyscallCpuLocal, exit_status);

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

global_asm!(r#"
.global systemes_load_gdt
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

.global systemes_load_tr
systemes_load_tr:
    mov ax, di
    ltr ax
    ret

.global systemes_enter_ring3
systemes_enter_ring3:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov qword ptr gs:[{gs_saved_kernel_rsp}], rsp
    push {user_data_selector}
    push rsi
    push rdx
    push {user_code_selector}
    push rdi
    swapgs
    iretq

.global systemes_ring3_exit_trampoline
systemes_ring3_exit_trampoline:
    mov ax, {kernel_data_selector}
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov rsp, qword ptr gs:[{gs_saved_kernel_rsp}]
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret

.global systemes_isr0
systemes_isr0:
    push 0
    push 0
    jmp systemes_exception_common

.global systemes_isr1
systemes_isr1:
    push 0
    push 1
    jmp systemes_exception_common

.global systemes_isr2
systemes_isr2:
    push 0
    push 2
    jmp systemes_exception_common

.global systemes_isr3
systemes_isr3:
    push 0
    push 3
    jmp systemes_exception_common

.global systemes_isr4
systemes_isr4:
    push 0
    push 4
    jmp systemes_exception_common

.global systemes_isr5
systemes_isr5:
    push 0
    push 5
    jmp systemes_exception_common

.global systemes_isr6
systemes_isr6:
    push 0
    push 6
    jmp systemes_exception_common

.global systemes_isr7
systemes_isr7:
    push 0
    push 7
    jmp systemes_exception_common

.global systemes_isr8
systemes_isr8:
    push 8
    jmp systemes_exception_common

.global systemes_isr9
systemes_isr9:
    push 0
    push 9
    jmp systemes_exception_common

.global systemes_isr10
systemes_isr10:
    push 10
    jmp systemes_exception_common

.global systemes_isr11
systemes_isr11:
    push 11
    jmp systemes_exception_common

.global systemes_isr12
systemes_isr12:
    push 12
    jmp systemes_exception_common

.global systemes_isr13
systemes_isr13:
    push 13
    jmp systemes_exception_common

.global systemes_isr14
systemes_isr14:
    push 14
    jmp systemes_exception_common

.global systemes_isr15
systemes_isr15:
    push 0
    push 15
    jmp systemes_exception_common

.global systemes_isr16
systemes_isr16:
    push 0
    push 16
    jmp systemes_exception_common

.global systemes_isr17
systemes_isr17:
    push 17
    jmp systemes_exception_common

.global systemes_isr18
systemes_isr18:
    push 0
    push 18
    jmp systemes_exception_common

.global systemes_isr19
systemes_isr19:
    push 0
    push 19
    jmp systemes_exception_common

.global systemes_isr20
systemes_isr20:
    push 0
    push 20
    jmp systemes_exception_common

.global systemes_isr21
systemes_isr21:
    push 21
    jmp systemes_exception_common

.global systemes_isr22
systemes_isr22:
    push 0
    push 22
    jmp systemes_exception_common

.global systemes_isr23
systemes_isr23:
    push 0
    push 23
    jmp systemes_exception_common

.global systemes_isr24
systemes_isr24:
    push 0
    push 24
    jmp systemes_exception_common

.global systemes_isr25
systemes_isr25:
    push 0
    push 25
    jmp systemes_exception_common

.global systemes_isr26
systemes_isr26:
    push 0
    push 26
    jmp systemes_exception_common

.global systemes_isr27
systemes_isr27:
    push 0
    push 27
    jmp systemes_exception_common

.global systemes_isr28
systemes_isr28:
    push 0
    push 28
    jmp systemes_exception_common

.global systemes_isr29
systemes_isr29:
    push 29
    jmp systemes_exception_common

.global systemes_isr30
systemes_isr30:
    push 30
    jmp systemes_exception_common

.global systemes_isr31
systemes_isr31:
    push 0
    push 31
    jmp systemes_exception_common

.global systemes_user_exit
systemes_user_exit:
    push 0
    push 128
    jmp systemes_exception_common

.global systemes_syscall_entry
systemes_syscall_entry:
    swapgs
    mov qword ptr gs:[{gs_user_rsp}], rsp
    mov rsp, qword ptr gs:[{gs_syscall_stack_top}]
    push qword ptr gs:[{gs_user_rsp}]
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
    mov rsp, qword ptr gs:[{gs_user_rsp}]
    swapgs
    sysretq
.Lsystemes_syscall_exit:
    mov rax, qword ptr gs:[{gs_exit_status}]
    jmp systemes_ring3_exit_trampoline

.global systemes_lapic_timer
systemes_lapic_timer:
    push 0
    push 64
    jmp systemes_irq_common

.global systemes_scheduler_yield
systemes_scheduler_yield:
    push 0
    push 65
    jmp systemes_irq_common

.global systemes_apic_spurious
systemes_apic_spurious:
    test byte ptr [rsp + 8], 3
    jz .Lsystemes_spurious_gs_ready
    swapgs
    swapgs
.Lsystemes_spurious_gs_ready:
    iretq

.global systemes_irq_common
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
    test byte ptr [rsp + 144], 3
    jz .Lsystemes_irq_gs_ready
    swapgs
.Lsystemes_irq_gs_ready:
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
    test byte ptr [rsp + 8], 3
    jz .Lsystemes_irq_gs_exit_ready
    swapgs
.Lsystemes_irq_gs_exit_ready:
    iretq

.global systemes_exception_common
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
    /* Paranoid exception entry: NMI/MCE may arrive inside a SWAPGS window.
       Read IA32_GS_BASE instead of inferring its state from saved CS. */
    mov ecx, 0xC0000101
    rdmsr
    test eax, eax
    jnz .Lsystemes_exception_gs_present
    test edx, edx
    jnz .Lsystemes_exception_gs_present
    swapgs
    mov eax, 1
    jmp .Lsystemes_exception_gs_mark
.Lsystemes_exception_gs_present:
    xor eax, eax
.Lsystemes_exception_gs_mark:
    /* bit 0: this entry performed SWAPGS; bit 1: original frame was CPL3. */
    test byte ptr [r12 + 144], 3
    jz .Lsystemes_exception_store_gs_marker
    or eax, 2
.Lsystemes_exception_store_gs_marker:
    push rax
    mov rdi, qword ptr [r12 + 120]
    mov rsi, qword ptr [r12 + 128]
    mov rdx, r12
    and rsp, -16
    call systemes_exception_dispatch
    lea rsp, [r12 - 8]
    test qword ptr [rsp], 1
    jz .Lsystemes_exception_gs_exit_ready
    /* A validated INT 0x80 exit rewrites CPL3 to CPL0: keep kernel GS. */
    test qword ptr [rsp], 2
    jz .Lsystemes_exception_swapgs_back
    test byte ptr [r12 + 144], 3
    jz .Lsystemes_exception_gs_exit_ready
.Lsystemes_exception_swapgs_back:
    swapgs
.Lsystemes_exception_gs_exit_ready:
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
    user_data_selector = const USER_DATA_RPL3,
    user_code_selector = const USER_CODE_RPL3,
    gs_syscall_stack_top = const GS_SYSCALL_STACK_TOP,
    gs_user_rsp = const GS_USER_RSP,
    gs_saved_kernel_rsp = const GS_SAVED_KERNEL_RSP,
    gs_exit_status = const GS_EXIT_STATUS,
);

/// Return true only when the processor loaded the kernel's 64-bit TSS selector.
pub fn verify_task_register() -> bool {
    let selector: u16;
    // SAFETY: STR is available at CPL0 and reads the current task-register selector.
    unsafe { asm!("str {0:x}", out(reg) selector, options(nomem, nostack, preserves_flags)); }
    selector == TSS_SELECTOR
}

/// Validate the live TSS stack pointers and the IST selectors installed in the IDT.
pub fn verify_ist_configuration(cpu_index: usize) -> bool {
    if cpu_index >= MAX_CPUS || CPU_INIT_STATE[cpu_index].load(Ordering::Acquire) != 2 {
        return false;
    }
    // SAFETY: CPU_CONTEXTS is a static array; we compute the element pointer manually.
    let ctx = unsafe { (addr_of!(CPU_CONTEXTS) as *const CpuContext).add(cpu_index) };
    let tss = unsafe { addr_of!((*ctx).tss) };
    let idt = unsafe { addr_of!((*ctx).idt) as *const IdtEntry };
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
pub fn verify_local_apic_gates(cpu_index: usize) -> bool {
    if cpu_index >= MAX_CPUS {
        return false;
    }
    // SAFETY: CPU_CONTEXTS is a static array; we compute the element pointer manually.
    let ctx = unsafe { (addr_of!(CPU_CONTEXTS) as *const CpuContext).add(cpu_index) };
    let idt = unsafe { addr_of!((*ctx).idt) as *const IdtEntry };
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
pub fn verify_ring3_configuration(cpu_index: usize) -> bool {
    if cpu_index >= MAX_CPUS {
        return false;
    }
    // SAFETY: CPU_CONTEXTS is a static array; we compute the element pointer manually.
    let ctx = unsafe { (addr_of!(CPU_CONTEXTS) as *const CpuContext).add(cpu_index) };
    let gdt = unsafe { addr_of!((*ctx).gdt).cast::<u64>() };
    let idt = unsafe { addr_of!((*ctx).idt) as *const IdtEntry };
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
/// Must run at CPL0 after this CPU's GS anchor and kernel GDT are installed,
/// and before user code can execute `SYSCALL`. Call once on every CPU.
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
    verify_syscall_configuration_for_cpu(current_cpu_index())
}

/// Verify an explicitly assigned CPU slot without relying on Local APIC MMIO.
/// APs call this immediately after their GS anchor is installed, before their
/// Local APIC timer setup is complete.
pub(crate) fn verify_syscall_configuration_for_cpu(cpu_index: usize) -> bool {
    if cpu_index >= MAX_CPUS {
        return false;
    }
    let local = unsafe { addr_of!(CPU_CONTEXTS[cpu_index].syscall) };
    let expected_gs = local as u64;
    let stack_top = unsafe { addr_of!((*local).syscall_stack_top).read() };
    read_msr(MSR_EFER) & EFER_SCE != 0
        && read_msr(MSR_STAR) == STAR_VALUE
        && read_msr(MSR_LSTAR) == systemes_syscall_entry as *const () as usize as u64
        && read_msr(MSR_SFMASK) == SFMASK_VALUE
        && read_msr(MSR_GS_BASE) == expected_gs
        && read_msr(MSR_KERNEL_GS_BASE) == 0
        && stack_top != 0
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
#[cfg(any(feature = "qemu-ring3-test", feature = "qemu-ring3-smp-test"))]
pub(crate) unsafe fn enter_ring3_test_context(user_rip: u64, user_rsp: u64) -> bool {
    let local = current_syscall_local();
    // SAFETY: The current CPU exclusively owns these atomic per-CPU fields.
    unsafe {
        (*local).exit_validated.store(false, Ordering::Release);
        (*local).expected_user_rsp.store(user_rsp, Ordering::Release);
        (*local).ring3_active.store(true, Ordering::Release);
        (*local).exit_status.store(0, Ordering::Release);
    }
    // SAFETY: The caller guarantees valid user mappings and a configured TSS.
    unsafe { systemes_enter_ring3(user_rip, user_rsp, USER_TEST_RFLAGS) };
    // SAFETY: The current CPU exclusively owns these atomic per-CPU fields.
    unsafe {
        (*local).ring3_active.store(false, Ordering::Release);
        (*local).exit_validated.load(Ordering::Acquire)
    }
}

/// Enter ring 3 at `rip` with `rsp` as the user stack. Returns the status passed
/// to `SYS_EXIT`; a recoverable CPL3 #PF/#GP returns USER_FAULT_EXIT_STATUS.
///
/// # Safety
///
/// The caller must have installed the GDT/IDT/TSS, mapped `rip` executable
/// and `rsp` writable in the active address space, and disabled interrupts.
pub unsafe fn enter_user_mode(rip: u64, rsp: u64) -> u64 {
    let local = current_syscall_local();
    unsafe {
        (*local).exit_status.store(0, Ordering::Release);
        (*local).ring3_active.store(true, Ordering::Release);
        (*local).fault_vector.store(0, Ordering::Release);
        (*local).fault_error.store(0, Ordering::Release);
        (*local).fault_cr2.store(0, Ordering::Release);
        (*local).fault_rip.store(0, Ordering::Release);
    }
    // SAFETY: The caller guarantees valid user mappings and a configured TSS.
    unsafe { systemes_enter_ring3(rip, rsp, USER_MODE_RFLAGS) };
    unsafe { (*local).ring3_active.store(false, Ordering::Release); }
    crate::interrupts::user_exit_status()
}

/// Per-CPU syscall exit status (for ring-3 test compatibility).
#[cfg(test)]
fn current_syscall_local() -> *mut SyscallCpuLocal {
    // Host-side unit tests run at CPL3 and cannot execute RDMSR. Their isolated
    // test context uses the bootstrap CPU slot instead of the hardware GS base.
    let cpu_index = current_cpu_index();
    assert!(cpu_index < MAX_CPUS, "test CPU index exceeds supported slots");
    unsafe { addr_of_mut!(CPU_CONTEXTS[cpu_index].syscall) }
}

#[cfg(not(test))]
fn current_syscall_local() -> *mut SyscallCpuLocal {
    let local = read_msr(MSR_GS_BASE) as *mut SyscallCpuLocal;
    assert!(!local.is_null(), "kernel GS context is not initialized");
    // SAFETY: Every kernel entry path establishes GS_BASE before dispatch. The
    // raw pointer avoids creating a reference while assembly updates its stack.
    local
}

pub(crate) fn set_user_exit_status(status: u64) {
    // SAFETY: The current CPU exclusively owns its local syscall context.
    unsafe { (*current_syscall_local()).exit_status.store(status, Ordering::Release) };
}

pub(crate) fn user_exit_status() -> u64 {
    // SAFETY: The current CPU exclusively owns its local syscall context.
    unsafe { (*current_syscall_local()).exit_status.load(Ordering::Acquire) }
}

/// Return the most recent recoverable CPL3 fault on this CPU, if any.
pub fn last_user_fault() -> Option<UserFaultInfo> {
    let local = current_syscall_local();
    // SAFETY: The current CPU owns its GS-local fault record.
    let vector = unsafe { (*local).fault_vector.load(Ordering::Acquire) };
    if vector == 0 {
        return None;
    }
    // SAFETY: The record is published before the continuation is resumed.
    Some(unsafe {
        UserFaultInfo {
            vector,
            error_code: (*local).fault_error.load(Ordering::Acquire),
            cr2: (*local).fault_cr2.load(Ordering::Acquire),
            rip: (*local).fault_rip.load(Ordering::Acquire),
        }
    })
}

/// Return the bootstrap CPU's Local APIC ID from CPUID before MMIO APIC setup.
pub fn bootstrap_cpu_index() -> usize {
    (core::arch::x86_64::__cpuid(1).ebx >> 24) as usize
}

fn current_cpu_index() -> usize {
    crate::apic::current_apic_id()
        .map(|id| id as usize)
        .unwrap_or_else(bootstrap_cpu_index)
}

/// Initialize this CPU's private GDT, IDT, and TSS.
///
/// # Safety
///
/// Must run at CPL0 with interrupts disabled. Each CPU calls this once during
/// its bring-up. The caller must ensure `cpu_index` is unique per CPU.
pub unsafe fn initialize_cpu(cpu_index: usize) {
    if cpu_index >= MAX_CPUS {
        return;
    }
    if CPU_INIT_STATE[cpu_index]
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        while CPU_INIT_STATE[cpu_index].load(Ordering::Acquire) == 1 {
            core::hint::spin_loop();
        }
        return;
    }
    // SAFETY: This CPU owns the unique APIC-ID slot; keep local interrupts off
    // until its private GDT, TSS, and IDT are fully published.
    unsafe { asm!("cli", options(nomem, nostack)) };

    // SAFETY: CPU_CONTEXTS is a static array; we compute the element pointer manually.
    let ctx = unsafe { (addr_of_mut!(CPU_CONTEXTS) as *mut CpuContext).add(cpu_index) };

    // Establish the invariant: kernel GS is this CPU's context; user GS is zero.
    let syscall_local = unsafe { addr_of_mut!((*ctx).syscall) };
    let syscall_stack_top = unsafe {
        addr_of_mut!((*syscall_local).syscall_stack)
            .cast::<u8>()
            .add(INTERRUPT_STACK_SIZE) as u64
    };
    unsafe { addr_of_mut!((*syscall_local).syscall_stack_top).write(syscall_stack_top) };
    write_msr(MSR_GS_BASE, syscall_local as u64);
    write_msr(MSR_KERNEL_GS_BASE, 0);

    // Initialize independent emergency stacks before exposing IST-backed gates.
    let rsp0_top = unsafe { addr_of_mut!((*ctx).privilege_stack.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64 };
    let df_top = unsafe { addr_of_mut!((*ctx).double_fault_stack.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64 };
    let nmi_top = unsafe { addr_of_mut!((*ctx).nmi_stack.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64 };
    let mc_top = unsafe { addr_of_mut!((*ctx).machine_check_stack.0).cast::<u8>().add(INTERRUPT_STACK_SIZE) as u64 };
    let tss = unsafe { addr_of_mut!((*ctx).tss) };
    unsafe {
        core::ptr::addr_of_mut!((*tss).rsp0).write_unaligned(rsp0_top);
        core::ptr::addr_of_mut!((*tss).ist1).write_unaligned(df_top);
        core::ptr::addr_of_mut!((*tss).ist2).write_unaligned(nmi_top);
        core::ptr::addr_of_mut!((*tss).ist3).write_unaligned(mc_top);
        core::ptr::addr_of_mut!((*tss).iomap_base).write_unaligned(core::mem::size_of::<TaskStateSegment>() as u16);
    }

    let tss_base = tss as u64;
    let [tss_low, tss_high] = encode_tss_descriptor(tss_base, (core::mem::size_of::<TaskStateSegment>() - 1) as u32);
    let gdt = unsafe { addr_of_mut!((*ctx).gdt).cast::<u64>() };
    unsafe {
        gdt.add(0).write(0); // null descriptor
        gdt.add(1).write(0x00AF_9A00_0000_FFFF); // kernel code
        gdt.add(2).write(0x00CF_9200_0000_FFFF); // kernel data
        gdt.add(3).write(tss_low);
        gdt.add(4).write(tss_high);
        gdt.add(5).write(USER_DATA_DESCRIPTOR);
        gdt.add(6).write(USER_CODE_DESCRIPTOR);
    }
    let gdt_pointer = DescriptorTablePointer {
        limit: (core::mem::size_of::<[u64; 7]>() - 1) as u16,
        base: gdt as u64,
    };
    // SAFETY: GDT/TSS are static, identity-mapped, and remain alive for the kernel lifetime.
    unsafe { systemes_load_gdt(&gdt_pointer) };
    // SAFETY: The TSS descriptor at selector 0x18 is present and references this CPU's static TSS.
    unsafe { systemes_load_tr(TSS_SELECTOR) };

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
    let idt = unsafe { addr_of_mut!((*ctx).idt) as *mut IdtEntry };
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

    CPU_INIT_STATE[cpu_index].store(2, Ordering::Release);
}

/// Initialize the bootstrap CPU's descriptor tables (legacy entry point for compatibility).
pub unsafe fn initialize() {
    // SAFETY: Called during early boot on BSP only.
    unsafe { initialize_cpu(bootstrap_cpu_index()) };
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
        let cpu_index = current_cpu_index();
        let start = unsafe { addr_of!(CPU_CONTEXTS[cpu_index].nmi_stack.0) as *const u8 as u64 };
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
        let cpu_index = current_cpu_index();
        let start = unsafe { addr_of!(CPU_CONTEXTS[cpu_index].machine_check_stack.0) as *const u8 as u64 };
        let end = start + INTERRUPT_STACK_SIZE as u64;
        unsafe {
            if rsp >= start && rsp < end {
                debug_write(b"SYSTEMES_TEST: IST3 machine-check stack switch verified\r\n");
            } else {
                debug_write(b"SYSTEMES_KERNEL_FAIL: IST3 stack switch missing\r\n");
            }
        }
    }

    let syscall_local = current_syscall_local();
    // CPL is read from the processor-saved CS. ring3_active is only an
    // additional guard for the known synchronous enter_user_mode continuation.
    if let Some(frame_ref) = unsafe { frame.as_mut() } {
        if frame_ref.cs & 3 == 3 && matches!(vector, 13 | 14)
            && unsafe { (*syscall_local).ring3_active.load(Ordering::Acquire) }
        {
            let privilege_frame = frame.cast::<u64>();
            let user_ss = unsafe { privilege_frame.add(21).read() };
            if user_ss == USER_DATA_RPL3 as u64 {
                let cr2 = if vector == 14 {
                    let value: u64;
                    unsafe { asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags)); }
                    value
                } else { 0 };
                unsafe {
                    (*syscall_local).fault_vector.store(vector, Ordering::Release);
                    (*syscall_local).fault_error.store(error_code, Ordering::Release);
                    (*syscall_local).fault_cr2.store(cr2, Ordering::Release);
                    (*syscall_local).fault_rip.store(frame_ref.rip, Ordering::Release);
                    (*syscall_local).exit_status.store(USER_FAULT_EXIT_STATUS, Ordering::Release);
                    (*syscall_local).ring3_active.store(false, Ordering::Release);
                }
                frame_ref.rip = systemes_ring3_exit_trampoline as *const () as usize as u64;
                frame_ref.cs = KERNEL_CODE_SELECTOR as u64;
                frame_ref.rflags = 0x02;
                let kernel_rsp = unsafe { (*syscall_local).saved_kernel_rsp.load(Ordering::Acquire) };
                unsafe {
                    privilege_frame.add(20).write(kernel_rsp);
                    privilege_frame.add(21).write(KERNEL_DATA_SELECTOR as u64);
                    debug_write(b"SYSTEMES_TEST: Ring 3 fault detected vector=0x");
                    debug_hex(vector);
                    debug_write(b" error=0x");
                    debug_hex(error_code);
                    debug_write(b" cr2=0x");
                    debug_hex(cr2);
                    debug_write(b" rip=0x");
                    debug_hex((*syscall_local).fault_rip.load(Ordering::Acquire));
                    debug_write(b"\r\n");
                }
                return;
            }
        }
    }
    // CPL0 faults (including syscall/UACCESS faults) remain fatal.
    if vector == USER_EXIT_VECTOR && unsafe { (*syscall_local).ring3_active.load(Ordering::Acquire) } {
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
                    unsafe { (*syscall_local).expected_user_rsp.load(Ordering::Acquire) },
                )
            {
                frame_ref.rip = systemes_ring3_exit_trampoline as *const () as usize as u64;
                frame_ref.cs = KERNEL_CODE_SELECTOR as u64;
                frame_ref.rflags = 0x02;
                let kernel_rsp = unsafe {
                    (*syscall_local).saved_kernel_rsp.load(Ordering::Acquire)
                };
                unsafe {
                    privilege_frame.add(20).write(kernel_rsp);
                    privilege_frame.add(21).write(KERNEL_DATA_SELECTOR as u64);
                }
                unsafe {
                    (*syscall_local).ring3_active.store(false, Ordering::Release);
                    (*syscall_local).exit_validated.store(true, Ordering::Release);
                }
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

const fn encode_tss_descriptor(base: u64, limit: u32) -> [u64; 2] {
    let low = (limit as u64 & 0xFFFF)
        | ((base & 0x00FF_FFFF) << 16)
        | (0x89u64 << 40)
        | ((((limit as u64) >> 16) & 0x0F) << 48)
        | (((base >> 24) & 0xFF) << 56);
    [low, base >> 32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syscall_cpu_local_layout_matches_gs_assembly_offsets() {
        assert_eq!(core::mem::align_of::<SyscallCpuLocal>(), 16);
        assert_eq!(GS_SYSCALL_STACK_TOP, INTERRUPT_STACK_SIZE);
        assert_eq!(GS_USER_RSP, INTERRUPT_STACK_SIZE + 8);
        assert_eq!(GS_SAVED_KERNEL_RSP, INTERRUPT_STACK_SIZE + 16);
        assert_eq!(core::mem::offset_of!(SyscallCpuLocal, ring3_active), INTERRUPT_STACK_SIZE + 24);
        assert_eq!(core::mem::offset_of!(SyscallCpuLocal, exit_validated), INTERRUPT_STACK_SIZE + 25);
        assert_eq!(core::mem::offset_of!(SyscallCpuLocal, expected_user_rsp), INTERRUPT_STACK_SIZE + 32);
        assert_eq!(GS_EXIT_STATUS, INTERRUPT_STACK_SIZE + 40);
    }

    #[test]
    fn syscall_cpu_local_state_is_independent_between_instances() {
        let first = SyscallCpuLocal::new();
        let second = SyscallCpuLocal::new();
        first.user_rsp.store(0x1010, Ordering::Release);
        second.user_rsp.store(0x2020, Ordering::Release);
        first.saved_kernel_rsp.store(0x1111, Ordering::Release);
        second.saved_kernel_rsp.store(0x2222, Ordering::Release);
        first.ring3_active.store(true, Ordering::Release);
        second.ring3_active.store(false, Ordering::Release);
        first.exit_validated.store(false, Ordering::Release);
        second.exit_validated.store(true, Ordering::Release);
        first.expected_user_rsp.store(0x3333, Ordering::Release);
        second.expected_user_rsp.store(0x4444, Ordering::Release);
        first.exit_status.store(7, Ordering::Release);
        second.exit_status.store(9, Ordering::Release);
        assert_eq!(first.user_rsp.load(Ordering::Acquire), 0x1010);
        assert_eq!(second.user_rsp.load(Ordering::Acquire), 0x2020);
        assert_eq!(first.saved_kernel_rsp.load(Ordering::Acquire), 0x1111);
        assert_eq!(second.saved_kernel_rsp.load(Ordering::Acquire), 0x2222);
        assert!(first.ring3_active.load(Ordering::Acquire));
        assert!(!second.ring3_active.load(Ordering::Acquire));
        assert!(!first.exit_validated.load(Ordering::Acquire));
        assert!(second.exit_validated.load(Ordering::Acquire));
        assert_eq!(first.expected_user_rsp.load(Ordering::Acquire), 0x3333);
        assert_eq!(second.expected_user_rsp.load(Ordering::Acquire), 0x4444);
        assert_eq!(first.exit_status.load(Ordering::Acquire), 7);
        assert_eq!(second.exit_status.load(Ordering::Acquire), 9);
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
        let df = IdtEntry::interrupt_gate(0x1000, 1);
        let nmi = IdtEntry::interrupt_gate(0x2000, 2);
        let mc = IdtEntry::interrupt_gate(0x3000, 3);
        assert_eq!((df.ist, nmi.ist, mc.ist), (1, 2, 3));
    }

    #[test]
    fn ring3_entry_enables_interrupts_for_the_monotonic_clock() {
        assert_ne!(USER_MODE_RFLAGS & (1 << 9), 0);
        assert_ne!(USER_MODE_RFLAGS & 0x02, 0);
        assert_eq!(USER_TEST_RFLAGS & (1 << 9), 0);
        assert_ne!(USER_TEST_RFLAGS & 0x02, 0);
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