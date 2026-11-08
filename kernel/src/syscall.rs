//! Minimal x86_64 syscall ABI for the single-core user-mode bring-up.

use core::arch::asm;

use crate::{
    address_space::{is_user_address, user_range_is_mapped},
    console::debug_write,
    uaccess::is_user_range,
};
#[cfg(feature = "qemu-user-shell-test")]
use core::sync::atomic::{AtomicBool, Ordering};

pub const SYS_GET_API_VERSION: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_READ: u64 = 2;
pub const SYS_EXIT: u64 = 3;
pub const SYS_YIELD: u64 = 4;
pub const API_VERSION: u64 = 1;
const ERROR_NO_SYS: u64 = (-38i64) as u64;
const ERROR_BAD_FD: u64 = (-9i64) as u64;
const ERROR_BAD_ADDRESS: u64 = (-14i64) as u64;
const ERROR_IO: u64 = (-5i64) as u64;
const USER_FLAGS_MASK: u64 = 0xAD5;

#[cfg(feature = "qemu-user-shell-test")]
static USER_SHELL_WRITE_SEEN: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "qemu-user-shell-test")]
static USER_SHELL_READ_SEEN: AtomicBool = AtomicBool::new(false);

#[repr(C)]
pub(crate) struct SyscallFrame {
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
    pub user_rsp: u64,
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn systemes_syscall_dispatch(frame: *mut SyscallFrame) -> u64 {
    if frame.is_null() {
        syscall_halt(b"SYSTEMES_KERNEL_FAIL: null syscall frame\r\n");
    }

    // SAFETY: The assembly entry point passes a pointer to its fixed kernel-stack frame.
    let frame = unsafe { &mut *frame };
    if !valid_return_context(frame) {
        syscall_halt(b"SYSTEMES_KERNEL_FAIL: invalid syscall return context\r\n");
    }

    frame.r11 = (frame.r11 & USER_FLAGS_MASK) | 0x02;
    match frame.rax {
        SYS_GET_API_VERSION => API_VERSION,
        SYS_WRITE => {
            if frame.rdi != 1 {
                ERROR_BAD_FD
            } else {
                let buf = frame.rsi;
                let len = frame.rdx;
                if !is_user_range(buf, len)
                    || !unsafe { user_range_is_mapped(buf, len, false) }
                {
                    ERROR_BAD_ADDRESS
                } else {
                    let _guard = crate::uaccess::UserAccessGuard::new();
                    let mut written: u64 = 0;
                    while written < len {
                        let byte = unsafe { core::ptr::read_volatile((buf + written) as *const u8) };
                        if unsafe { crate::serial::write_byte(byte) }.is_err() {
                            return if written == 0 { ERROR_IO } else { written };
                        }
                        written += 1;
                    }
                    #[cfg(feature = "qemu-user-shell-test")]
                    if written != 0 && !USER_SHELL_WRITE_SEEN.swap(true, Ordering::Relaxed) {
                        unsafe { debug_write(b"SYSTEMES_TEST: user SYS_WRITE succeeded\r\n") };
                    }
                    written
                }
            }
        }
        SYS_READ => {
            if frame.rdi != 1 {
                ERROR_BAD_FD
            } else {
                let buf = frame.rsi;
                let len = frame.rdx;
                if !is_user_range(buf, len)
                    || !unsafe { user_range_is_mapped(buf, len, true) }
                {
                    ERROR_BAD_ADDRESS
                } else {
                    let _guard = crate::uaccess::UserAccessGuard::new();
                    let mut read: u64 = 0;
                    while read < len {
                        match unsafe { crate::serial::read_byte() } {
                            Ok(Some(byte)) => {
                                unsafe {
                                    core::ptr::write_volatile((buf + read) as *mut u8, byte)
                                };
                                read += 1;
                                if byte == b'\n' || byte == b'\r' {
                                    break;
                                }
                            }
                            Ok(None) => core::hint::spin_loop(),
                            Err(_) => return if read == 0 { ERROR_IO } else { read },
                        }
                    }
                    #[cfg(feature = "qemu-user-shell-test")]
                    if read != 0 && !USER_SHELL_READ_SEEN.swap(true, Ordering::Relaxed) {
                        unsafe { debug_write(b"SYSTEMES_TEST: user SYS_READ received UART input\r\n") };
                    }
                    read
                }
            }
        }
        SYS_EXIT => {
            // SAFETY: The frame has a valid return context; exiting user mode
            // returns control to the kernel scheduler.
            unsafe { core::arch::asm!("cli; hlt") };
            0
        }
        SYS_YIELD => {
            // Trigger yield via the APIC timer interrupt vector 0x41
            // SAFETY: The frame has a valid return context; yielding returns
            // control to the kernel scheduler through the existing interrupt
            // vector that calls scheduler::on_yield.
            unsafe { core::arch::asm!("int3") };
            0
        }
        _ => ERROR_NO_SYS,
    }
}

fn valid_return_context(frame: &SyscallFrame) -> bool {
    is_user_address(frame.rcx) && is_user_address(frame.user_rsp)
}

fn syscall_halt(message: &[u8]) -> ! {
    unsafe { debug_write(message) };
    loop {
        // SAFETY: An invalid syscall return context must not return to user mode.
        unsafe {
            asm!("cli; hlt", options(nomem, nostack));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_frame(number: u64) -> SyscallFrame {
        SyscallFrame {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: u64::MAX,
            r10: 0,
            r9: 0,
            r8: 0,
            rbp: 0,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: crate::address_space::USER_BASE + 0x4000,
            rbx: 0,
            rax: number,
            user_rsp: crate::address_space::USER_BASE + 0x8000,
        }
    }

    #[test]
    fn dispatches_api_version_and_sanitizes_return_flags() {
        let mut frame = valid_frame(SYS_GET_API_VERSION);
        assert_eq!(systemes_syscall_dispatch(&mut frame), API_VERSION);
        assert_eq!(frame.r11, USER_FLAGS_MASK | 0x02);
    }

    #[test]
    fn reports_unknown_syscalls_with_enosys() {
        let mut frame = valid_frame(u64::MAX);
        assert_eq!(systemes_syscall_dispatch(&mut frame), ERROR_NO_SYS);
    }

    #[test]
    fn rejects_kernel_and_noncanonical_return_addresses() {
        let mut frame = valid_frame(SYS_GET_API_VERSION);
        frame.rcx = crate::address_space::USER_BASE - 1;
        assert!(!valid_return_context(&frame));
        frame.rcx = crate::address_space::USER_LIMIT;
        assert!(!valid_return_context(&frame));
        frame.rcx = crate::address_space::USER_BASE + 0x4000;
        frame.user_rsp = crate::address_space::USER_LIMIT;
        assert!(!valid_return_context(&frame));
    }

    #[test]
    fn syscall_frame_matches_the_assembly_push_order() {
        assert_eq!(core::mem::size_of::<SyscallFrame>(), 16 * 8);
        assert_eq!(core::mem::offset_of!(SyscallFrame, r15), 0);
        assert_eq!(core::mem::offset_of!(SyscallFrame, rax), 112);
        assert_eq!(core::mem::offset_of!(SyscallFrame, user_rsp), 120);
    }

    #[test]
    fn write_syscall_rejects_bad_fd() {
        let mut frame = valid_frame(SYS_WRITE);
        frame.rdi = 2;
        frame.rsi = crate::address_space::USER_BASE + 0x4000;
        frame.rdx = 5;
        assert_eq!(systemes_syscall_dispatch(&mut frame), ERROR_BAD_FD);
    }

    #[test]
    fn write_syscall_rejects_non_user_buffer() {
        let mut frame = valid_frame(SYS_WRITE);
        frame.rdi = 1;
        frame.rsi = crate::address_space::USER_BASE - 1;
        frame.rdx = 5;
        assert_eq!(systemes_syscall_dispatch(&mut frame), ERROR_BAD_ADDRESS);
    }

    #[test]
    fn write_syscall_rejects_non_user_length() {
        let mut frame = valid_frame(SYS_WRITE);
        frame.rdi = 1;
        frame.rsi = crate::address_space::USER_LIMIT - 4;
        frame.rdx = 8;
        assert_eq!(systemes_syscall_dispatch(&mut frame), ERROR_BAD_ADDRESS);
    }

    #[test]
    fn read_syscall_rejects_bad_fd() {
        let mut frame = valid_frame(SYS_READ);
        frame.rdi = 2;
        frame.rsi = crate::address_space::USER_BASE + 0x4000;
        frame.rdx = 5;
        assert_eq!(systemes_syscall_dispatch(&mut frame), ERROR_BAD_FD);
    }

    #[test]
    fn read_syscall_rejects_non_user_buffer() {
        let mut frame = valid_frame(SYS_READ);
        frame.rdi = 1;
        frame.rsi = crate::address_space::USER_BASE - 1;
        frame.rdx = 5;
        assert_eq!(systemes_syscall_dispatch(&mut frame), ERROR_BAD_ADDRESS);
    }

    #[test]
    fn read_syscall_rejects_non_user_length() {
        let mut frame = valid_frame(SYS_READ);
        frame.rdi = 1;
        frame.rsi = crate::address_space::USER_LIMIT - 4;
        frame.rdx = 8;
        assert_eq!(systemes_syscall_dispatch(&mut frame), ERROR_BAD_ADDRESS);
    }
}
