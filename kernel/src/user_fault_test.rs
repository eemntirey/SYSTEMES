//! Boot-time proof that a Ring 3 CPU exception kills only the offending process.
//!
//! The test launches a user program that writes to a deliberately unmapped user
//! page, then verifies the kernel regained control, recorded the #PF vector,
//! reclaimed every process frame, and can launch a second, healthy process.

use core::arch::asm;

use crate::address_space::{
    cr3_root, read_cr3, write_cr3, AddressSpace, UserPagePermissions, USER_BASE,
};
use crate::console::{debug_hex, debug_write, encode_decimal};
use crate::interrupts::{
    enter_user_mode, user_fault_exit_status, user_fault_vector, PAGE_FAULT_VECTOR, NO_USER_FAULT,
};
use crate::memory::{PhysicalFrameAllocator, PAGE_SIZE};
use crate::paging::IDENTITY_MAP_BYTES;
use crate::syscall::SYS_EXIT;

const CODE_ADDRESS: u64 = USER_BASE + 0x400_0000;
const STACK_ADDRESS: u64 = USER_BASE + 0x400_1000;
/// Inside the user window but never mapped: a Ring 3 write must raise #PF.
const UNMAPPED_ADDRESS: u64 = USER_BASE + 0x500_0000;
const SECOND_PROCESS_EXIT_STATUS: u64 = 7;

const FAIL_FRAME: &[u8] = b"SYSTEMES_KERNEL_FAIL: user fault test frame unavailable\r\n";
const FAIL_ADDRESS_SPACE: &[u8] = b"SYSTEMES_KERNEL_FAIL: user fault test address space failed\r\n";
const FAIL_CR3: &[u8] = b"SYSTEMES_KERNEL_FAIL: user fault test CR3 switch failed\r\n";
const FAIL_RECLAIM: &[u8] = b"SYSTEMES_KERNEL_FAIL: user fault test frame reclaim failed\r\n";
const FAIL_WRONG_VECTOR: &[u8] = b"SYSTEMES_KERNEL_FAIL: user fault test reported the wrong vector\r\n";
const FAIL_WRONG_STATUS: &[u8] = b"SYSTEMES_KERNEL_FAIL: user fault test exit status mismatch\r\n";
const FAIL_FAULT_LEAKED: &[u8] = b"SYSTEMES_KERNEL_FAIL: user fault state leaked into the next process\r\n";
const FAIL_SECOND_PROCESS: &[u8] = b"SYSTEMES_KERNEL_FAIL: user fault test second process failed\r\n";

/// `mov rax, UNMAPPED_ADDRESS ; mov qword [rax], 0 ; ud2`.
///
/// The `ud2` is unreachable: the store must fault before it executes.
fn fault_program() -> ([u8; 19], usize) {
    let mut code = [0u8; 19];
    code[0] = 0x48;
    code[1] = 0xB8;
    code[2..10].copy_from_slice(&UNMAPPED_ADDRESS.to_le_bytes());
    code[10..17].copy_from_slice(&[0x48, 0xC7, 0x00, 0x00, 0x00, 0x00, 0x00]);
    code[17..19].copy_from_slice(&[0x0F, 0x0B]);
    (code, 19)
}

/// `mov eax, SYS_EXIT ; mov edi, status ; syscall`.
fn exit_program(status: u32) -> ([u8; 12], usize) {
    let mut code = [0u8; 12];
    code[0] = 0xB8;
    code[1..5].copy_from_slice(&(SYS_EXIT as u32).to_le_bytes());
    code[5] = 0xBF;
    code[6..10].copy_from_slice(&status.to_le_bytes());
    code[10..12].copy_from_slice(&[0x0F, 0x05]);
    (code, 12)
}

/// Load, enter and reclaim one user process in its own address space.
///
/// # Safety
///
/// Must run single-core with the kernel page tables active and interrupts
/// disabled; the caller must not launch another Ring 3 context concurrently.
unsafe fn run_program(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
    code: &[u8],
) -> Result<u64, &'static [u8]> {
    let free_pages_before = allocator.free_pages();
    let code_frame = allocator.allocate_frame().ok_or(FAIL_FRAME)?;
    let stack_frame = allocator.allocate_frame().ok_or(FAIL_FRAME)?;
    if code_frame % PAGE_SIZE != 0
        || stack_frame % PAGE_SIZE != 0
        || code_frame >= IDENTITY_MAP_BYTES
        || stack_frame >= IDENTITY_MAP_BYTES
    {
        return Err(FAIL_FRAME);
    }
    // SAFETY: Both frames are freshly allocated and identity-accessible.
    unsafe {
        core::ptr::write_bytes(code_frame as *mut u8, 0, PAGE_SIZE as usize);
        core::ptr::write_bytes(stack_frame as *mut u8, 0, PAGE_SIZE as usize);
        core::ptr::copy_nonoverlapping(code.as_ptr(), code_frame as *mut u8, code.len());
    }

    // SAFETY: The kernel root is active and the allocator owns free frames.
    let mut space =
        unsafe { AddressSpace::create(allocator, kernel_root) }.map_err(|_| FAIL_ADDRESS_SPACE)?;
    // SAFETY: The virtual addresses and backing frames were validated above.
    unsafe {
        space
            .map_user_page(
                allocator,
                CODE_ADDRESS,
                code_frame,
                UserPagePermissions::ReadOnlyExecute,
            )
            .map_err(|_| FAIL_ADDRESS_SPACE)?;
        space
            .map_user_page(
                allocator,
                STACK_ADDRESS,
                stack_frame,
                UserPagePermissions::ReadWriteNoExecute,
            )
            .map_err(|_| FAIL_ADDRESS_SPACE)?;
    }

    let user_rsp = STACK_ADDRESS + PAGE_SIZE - 16;
    let previous_root = unsafe { read_cr3() };
    // SAFETY: This test is single-core and must not be preempted while swapping CR3.
    unsafe {
        asm!("cli", options(nomem, nostack));
        write_cr3(space.root());
    }
    let result = if cr3_root(unsafe { read_cr3() }) != space.root() {
        Err(FAIL_CR3)
    } else {
        // SAFETY: The code page is user RX and the stack page is user RW-NX here.
        Ok(unsafe { enter_user_mode(CODE_ADDRESS, user_rsp) })
    };
    // SAFETY: Restore the kernel root before reclaiming the process frames.
    unsafe { write_cr3(previous_root) };
    // SAFETY: The kernel root is active, so none of the process frames are in use.
    let reclaimed = unsafe { space.destroy(allocator) }.is_ok()
        && allocator.free_pages() == free_pages_before;
    let status = result?;
    if !reclaimed {
        return Err(FAIL_RECLAIM);
    }
    Ok(status)
}

/// Prove Ring 3 fault recovery and post-fault process reclamation.
///
/// # Safety
///
/// Must run once during boot after the GDT/IDT/TSS, SYSCALL MSRs and kernel page
/// tables are active, with no other Ring 3 process running.
pub unsafe fn run(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
) -> Result<(), &'static [u8]> {
    // Preserve the caller's interrupt state: the CR3 swaps below run with IRQs
    // masked and must not leave the boot task non-preemptible.
    let caller_flags: u64;
    // SAFETY: Reading RFLAGS is valid at CPL0.
    unsafe {
        asm!("pushfq; pop {}", out(reg) caller_flags, options(nomem, nostack, preserves_flags));
    }
    let (fault_code, fault_len) = fault_program();
    // SAFETY: The boot contract above is satisfied by the caller.
    let status = unsafe { run_program(allocator, kernel_root, &fault_code[..fault_len]) }?;
    let vector = user_fault_vector();
    if vector != PAGE_FAULT_VECTOR {
        return Err(FAIL_WRONG_VECTOR);
    }
    if status != user_fault_exit_status(PAGE_FAULT_VECTOR) {
        return Err(FAIL_WRONG_STATUS);
    }
    // SAFETY: The debug console is initialized and the kernel is back in Ring 0.
    unsafe {
        debug_write(b"SYSTEMES_TEST: Ring 3 page fault killed only the user process\r\n");
        debug_write(b"SYSTEMES_TEST: fault exit status=");
        let mut digits = [0u8; 20];
        let length = encode_decimal(status, &mut digits);
        debug_write(&digits[..length]);
        debug_write(b" vector=0x");
        debug_hex(vector);
        debug_write(b"\r\n");
    }

    let (exit_code, exit_len) = exit_program(SECOND_PROCESS_EXIT_STATUS as u32);
    // SAFETY: The boot contract above is satisfied by the caller.
    let second_status =
        unsafe { run_program(allocator, kernel_root, &exit_code[..exit_len]) }?;
    if user_fault_vector() != NO_USER_FAULT {
        return Err(FAIL_FAULT_LEAKED);
    }
    if second_status != SECOND_PROCESS_EXIT_STATUS {
        return Err(FAIL_SECOND_PROCESS);
    }
    // SAFETY: The debug console is initialized and both test processes have exited.
    unsafe {
        debug_write(b"SYSTEMES_TEST: kernel launched a new user process after the fault\r\n");
        debug_write(b"SYSTEMES_TEST: faulted process frames reclaimed\r\n");
        if caller_flags & (1 << 9) != 0 {
            asm!("sti", options(nomem, nostack));
        }
    }
    Ok(())
}
