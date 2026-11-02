//! x86_64 descriptor tables and fatal CPU-exception diagnostics.
//!
//! The kernel remains in the UEFI-provided long-mode environment. This module
//! installs its own GDT and IDT but deliberately does not enable hardware IRQs.

use core::arch::{asm, global_asm};
use core::ptr::{addr_of, addr_of_mut};

const KERNEL_CODE_SELECTOR: u16 = 0x08;
const EXCEPTION_GATE_ATTRIBUTES: u8 = 0x8E;
const EXCEPTION_VECTOR_COUNT: usize = 32;

static GDT: [u64; 3] = [
    0,
    0x00AF_9A00_0000_FFFF, // Ring 0, present, executable, 64-bit code.
    0x00CF_9200_0000_FFFF, // Ring 0, present, writable data.
];

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

    const fn interrupt_gate(handler: u64) -> Self {
        Self {
            offset_low: handler as u16,
            selector: KERNEL_CODE_SELECTOR,
            ist: 0,
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
"#);

/// Install a private ring-0 GDT and gates for all architecturally defined
/// exception vectors 0..31. Hardware interrupts remain disabled.
pub unsafe fn initialize() {
    // SAFETY: This runs during single-core boot before the kernel enables IRQs.
    unsafe { asm!("cli", options(nomem, nostack)) };

    let gdt_pointer = DescriptorTablePointer {
        limit: (core::mem::size_of_val(&GDT) - 1) as u16,
        base: addr_of!(GDT) as u64,
    };
    // SAFETY: The GDT and pointer remain valid for the duration of this call.
    unsafe { systemes_load_gdt(&gdt_pointer) };

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
        // SAFETY: Each index is below EXCEPTION_VECTOR_COUNT and IDT has 256 entries.
        unsafe { idt.add(vector).write(IdtEntry::interrupt_gate(*handler as u64)) };
    }

    let idt_pointer = DescriptorTablePointer {
        limit: (EXCEPTION_VECTOR_COUNT * core::mem::size_of::<IdtEntry>() - 1) as u16,
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
    fn encodes_a_64_bit_ring_zero_interrupt_gate() {
        let entry = IdtEntry::interrupt_gate(0x1234_5678_9ABC_DEF0);
        let low = unsafe { addr_of!(entry.offset_low).read_unaligned() };
        let middle = unsafe { addr_of!(entry.offset_middle).read_unaligned() };
        let high = unsafe { addr_of!(entry.offset_high).read_unaligned() };
        assert_eq!(low, 0xDEF0);
        assert_eq!(middle, 0x9ABC);
        assert_eq!(high, 0x1234_5678);
        let selector = unsafe { addr_of!(entry.selector).read_unaligned() };
        let reserved = unsafe { addr_of!(entry.reserved).read_unaligned() };
        assert_eq!(selector, KERNEL_CODE_SELECTOR);
        assert_eq!(entry.ist, 0);
        assert_eq!(entry.attributes, EXCEPTION_GATE_ATTRIBUTES);
        assert_eq!(reserved, 0);
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
