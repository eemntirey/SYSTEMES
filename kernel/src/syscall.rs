//! Minimal x86_64 syscall ABI for the single-core user-mode bring-up.

use core::arch::asm;

use crate::{
    address_space::{is_user_address, user_range_is_mapped},
    console::debug_write,
    uaccess::is_user_range,
    file_descriptors::{FileTableError, ProcessFileTable},
    sync::SpinLock,
    vfs::VfsError,
};
use core::sync::atomic::{AtomicBool, Ordering};

pub const SYS_GET_API_VERSION: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_READ: u64 = 2;
pub const SYS_EXIT: u64 = 3;
pub const SYS_YIELD: u64 = 4;
pub const SYS_UPTIME: u64 = 5;
pub const SYS_OPEN: u64 = 6;
pub const SYS_CLOSE: u64 = 7;
pub const API_VERSION: u64 = 1;
pub const ERROR_NO_SYS: u64 = (-38i64) as u64;
pub const ERROR_BAD_FD: u64 = (-9i64) as u64;
pub const ERROR_BAD_ADDRESS: u64 = (-14i64) as u64;
pub const ERROR_IO: u64 = (-5i64) as u64;
pub const ERROR_NO_ENTRY: u64 = (-2i64) as u64;
pub const ERROR_NO_DEVICE: u64 = (-19i64) as u64;
pub const ERROR_INVALID_ARGUMENT: u64 = (-22i64) as u64;
pub const ERROR_TOO_MANY_FILES: u64 = (-24i64) as u64;
pub const ERROR_NAME_TOO_LONG: u64 = (-36i64) as u64;
pub const ERROR_NOT_DIRECTORY: u64 = (-20i64) as u64;
const MAX_OPEN_PATH: usize = 255;
const FILE_READ_CHUNK: usize = 512;
static PROCESS_FILES: SpinLock<ProcessFileTable> = SpinLock::new(ProcessFileTable::new());
pub(crate) const SYS_EXIT_DISPATCHED: u64 = u64::MAX;
const USER_FLAGS_MASK: u64 = 0xAD5;

#[cfg(not(test))]
static USER_SHELL_UPTIME_SEEN: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "qemu-user-shell-test")]
static USER_SHELL_WRITE_SEEN: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "qemu-user-shell-test")]
static USER_SHELL_READ_SEEN: AtomicBool = AtomicBool::new(false);

/// Validate the deliberately small path subset supported by the current SYSFS root.
fn validate_open_path(bytes: &[u8]) -> Result<&str, u64> {
    if bytes.is_empty() {
        return Err(ERROR_INVALID_ARGUMENT);
    }
    if bytes.len() > MAX_OPEN_PATH {
        return Err(ERROR_NAME_TOO_LONG);
    }
    if bytes[0] != b'/' || bytes.contains(&0) {
        return Err(ERROR_INVALID_ARGUMENT);
    }
    let path = core::str::from_utf8(bytes).map_err(|_| ERROR_INVALID_ARGUMENT)?;
    let component = &path[1..];
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.contains('/')
        || component.split('/').any(|part| part == ".." || part == ".")
    {
        return Err(ERROR_INVALID_ARGUMENT);
    }
    Ok(path)
}

fn map_vfs_error(error: VfsError) -> u64 {
    match error {
        VfsError::NotMounted => ERROR_NO_DEVICE,
        VfsError::NotFound => ERROR_NO_ENTRY,
        VfsError::NotAFile => ERROR_NOT_DIRECTORY,
        VfsError::InvalidPath => ERROR_INVALID_ARGUMENT,
        VfsError::NameTooLong => ERROR_NAME_TOO_LONG,
        VfsError::OutOfHandles => ERROR_TOO_MANY_FILES,
        VfsError::BadHandle => ERROR_BAD_FD,
        VfsError::Busy | VfsError::IoError => ERROR_IO,
    }
}

fn map_file_table_error(error: FileTableError) -> u64 {
    match error {
        FileTableError::BadDescriptor => ERROR_BAD_FD,
        FileTableError::TooManyOpenFiles => ERROR_TOO_MANY_FILES,
    }
}

/// Copy a bounded user path before parsing it; no user pointer escapes this function.
fn open_user_file(user_path: u64, path_len: u64) -> u64 {
    if path_len == 0 {
        return ERROR_INVALID_ARGUMENT;
    }
    if path_len > MAX_OPEN_PATH as u64 {
        return ERROR_NAME_TOO_LONG;
    }
    // SAFETY: The user range is bounded before the page-table walk; this only inspects mappings.
    let path_mapped = unsafe { user_range_is_mapped(user_path, path_len, false) };
    if !is_user_range(user_path, path_len) || !path_mapped {
        return ERROR_BAD_ADDRESS;
    }

    let mut path_bytes = [0u8; MAX_OPEN_PATH];
    {
        let _guard = crate::uaccess::UserAccessGuard::new();
        for (index, byte) in path_bytes[..path_len as usize].iter_mut().enumerate() {
            // SAFETY: The full user range was validated and mapped above; SMAP is scoped.
            *byte = unsafe { core::ptr::read_volatile((user_path + index as u64) as *const u8) };
        }
    }
    let path = match validate_open_path(&path_bytes[..path_len as usize]) {
        Ok(path) => path,
        Err(error) => return error,
    };

    let handle = match crate::rootfs::with_root(|vfs| vfs.open(path)) {
        Some(Ok(handle)) => handle,
        Some(Err(error)) => return map_vfs_error(error),
        None => return ERROR_NO_DEVICE,
    };
    match PROCESS_FILES.lock().insert(handle) {
        Ok(descriptor) => descriptor as u64,
        Err(error) => {
            let _ = crate::rootfs::with_root(|vfs| vfs.close(handle));
            map_file_table_error(error)
        }
    }
}

fn close_user_file(descriptor: u64) -> u64 {
    let descriptor = match u32::try_from(descriptor) {
        Ok(descriptor) if descriptor >= crate::file_descriptors::FIRST_FILE_DESCRIPTOR => descriptor,
        _ => return ERROR_BAD_FD,
    };
    let handle = match PROCESS_FILES.lock().get(descriptor) {
        Ok(handle) => handle,
        Err(error) => return map_file_table_error(error),
    };
    match crate::rootfs::with_root(|vfs| vfs.close(handle)) {
        Some(Ok(())) => {
            let _ = PROCESS_FILES.lock().remove(descriptor);
            0
        }
        Some(Err(error)) => map_vfs_error(error),
        None => ERROR_NO_DEVICE,
    }
}

/// Copy file bytes from the kernel VFS into a validated writable user range.
fn read_user_file(descriptor: u64, user_buffer: u64, length: u64) -> u64 {
    let descriptor = match u32::try_from(descriptor) {
        Ok(descriptor) if descriptor >= crate::file_descriptors::FIRST_FILE_DESCRIPTOR => descriptor,
        _ => return ERROR_BAD_FD,
    };
    if length == 0 {
        return match PROCESS_FILES.lock().get(descriptor) {
            Ok(_) => 0,
            Err(error) => map_file_table_error(error),
        };
    }
    // SAFETY: The user range is bounded before the page-table walk; this only inspects mappings.
    let buffer_mapped = unsafe { user_range_is_mapped(user_buffer, length, true) };
    if !is_user_range(user_buffer, length) || !buffer_mapped {
        return ERROR_BAD_ADDRESS;
    }
    let handle = match PROCESS_FILES.lock().get(descriptor) {
        Ok(handle) => handle,
        Err(error) => return map_file_table_error(error),
    };

    let mut total = 0u64;
    let mut chunk = [0u8; FILE_READ_CHUNK];
    while total < length {
        let chunk_len = core::cmp::min(length - total, FILE_READ_CHUNK as u64) as usize;
        let count = match crate::rootfs::with_root(|vfs| vfs.read(handle, &mut chunk[..chunk_len])) {
            Some(Ok(count)) => count,
            Some(Err(error)) => return if total == 0 { map_vfs_error(error) } else { total },
            None => return if total == 0 { ERROR_NO_DEVICE } else { total },
        };
        if count == 0 {
            break;
        }
        {
            let _guard = crate::uaccess::UserAccessGuard::new();
            for (index, byte) in chunk[..count].iter().copied().enumerate() {
                // SAFETY: The entire destination range was validated and mapped above.
                unsafe {
                    core::ptr::write_volatile((user_buffer + total + index as u64) as *mut u8, byte);
                }
            }
        }
        total += count as u64;
        if count < chunk_len {
            break;
        }
    }
    total
}

/// Release every VFS handle owned by the exiting user process.
fn close_process_files() {
    let handles = PROCESS_FILES.lock().take_all();
    let _ = crate::rootfs::with_root(|vfs| {
        for handle in handles.iter().flatten().copied() {
            let _ = vfs.close(handle);
        }
    });
}

pub fn reset_process_file_table() {
    PROCESS_FILES.lock().reset();
}

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
        SYS_YIELD => {
            crate::scheduler::yield_now();
            0
        }
        SYS_UPTIME => {
            let ticks = crate::clock::ticks();
            #[cfg(not(test))]
            if !USER_SHELL_UPTIME_SEEN.swap(true, Ordering::Relaxed) {
                unsafe { debug_write(b"SYSTEMES_TEST: user SYS_UPTIME succeeded\r\n") };
            }
            ticks
        }
        SYS_OPEN => open_user_file(frame.rdi, frame.rsi),
        SYS_CLOSE => close_user_file(frame.rdi),
        SYS_EXIT => {
            close_process_files();
            crate::interrupts::set_user_exit_status(frame.rdi);
            SYS_EXIT_DISPATCHED
        }
        SYS_WRITE => {
            if frame.rdi != 1 {
                ERROR_BAD_FD
            } else {
                let buf = frame.rsi;
                let len = frame.rdx;
                // SAFETY: The user range is bounded before the page-table walk; this only inspects mappings.
                let buffer_mapped = unsafe { user_range_is_mapped(buf, len, false) };
                if !is_user_range(buf, len) || !buffer_mapped {
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
            if frame.rdi == 1 {
                let buf = frame.rsi;
                let len = frame.rdx;
                if len == 0 {
                    0
                } else {
                    // SAFETY: The user range is bounded before the page-table walk; this only inspects mappings.
                    let buffer_mapped = unsafe { user_range_is_mapped(buf, len, true) };
                    if !is_user_range(buf, len) || !buffer_mapped {
                        return ERROR_BAD_ADDRESS;
                    }
                    let mut read: u64 = 0;
                    // SAFETY: This enables timer IRQs while the polled UART read waits.
                    unsafe { asm!("sti", options(nomem, nostack)); }
                    while read < len {
                        // SAFETY: The serial driver owns the UART port access.
                        match unsafe { crate::serial::read_byte() } {
                            Ok(Some(byte)) => {
                                {
                                    let _guard = crate::uaccess::UserAccessGuard::new();
                                    // SAFETY: The complete destination range was validated above.
                                    unsafe { core::ptr::write_volatile((buf + read) as *mut u8, byte) };
                                }
                                read += 1;
                                if byte == b'\n' || byte == b'\r' {
                                    break;
                                }
                            }
                            Ok(None) => core::hint::spin_loop(),
                            Err(_) => {
                                // SAFETY: Restore the syscall-entry interrupt mask before returning.
                                unsafe { asm!("cli", options(nomem, nostack)); }
                                return if read == 0 { ERROR_IO } else { read };
                            },
                        }
                    }
                    // SAFETY: Restore the syscall-entry interrupt mask before returning.
                    unsafe { asm!("cli", options(nomem, nostack)); }
                    #[cfg(feature = "qemu-user-shell-test")]
                    if read != 0 && !USER_SHELL_READ_SEEN.swap(true, Ordering::Relaxed) {
                        unsafe { debug_write(b"SYSTEMES_TEST: user SYS_READ received UART input\r\n") };
                    }
                    read
                }
            } else if frame.rdi >= crate::file_descriptors::FIRST_FILE_DESCRIPTOR as u64 {
                read_user_file(frame.rdi, frame.rsi, frame.rdx)
            } else {
                ERROR_BAD_FD
            }
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
    use std::vec;

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
        let mut frame = valid_frame(99);
        assert_eq!(systemes_syscall_dispatch(&mut frame), ERROR_NO_SYS);
    }

    #[test]
    fn yields_to_the_scheduler_and_returns_success() {
        let mut frame = valid_frame(SYS_YIELD);
        assert_eq!(systemes_syscall_dispatch(&mut frame), 0);
    }

    #[test]
    fn exit_dispatch_records_status_and_requests_kernel_return() {
        let mut frame = valid_frame(SYS_EXIT);
        frame.rdi = 23;
        assert_eq!(systemes_syscall_dispatch(&mut frame), SYS_EXIT_DISPATCHED);
        assert_eq!(crate::interrupts::user_exit_status(), 23);
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

    #[test]
    fn open_path_requires_an_absolute_single_component_utf8_path() {
        assert_eq!(validate_open_path(b"/hello.txt"), Ok("/hello.txt"));
        assert_eq!(validate_open_path(b"hello.txt"), Err(ERROR_INVALID_ARGUMENT));
        assert_eq!(validate_open_path(b"/../hello.txt"), Err(ERROR_INVALID_ARGUMENT));
        assert_eq!(validate_open_path(b"/docs/hello.txt"), Err(ERROR_INVALID_ARGUMENT));
        assert_eq!(validate_open_path(b"/bad\xFF"), Err(ERROR_INVALID_ARGUMENT));
        assert_eq!(validate_open_path(b"/"), Err(ERROR_INVALID_ARGUMENT));
        assert_eq!(validate_open_path(&vec![b'a'; 256]), Err(ERROR_NAME_TOO_LONG));
    }

    #[test]
    fn maps_vfs_errors_to_stable_errno_values() {
        assert_eq!(map_vfs_error(VfsError::NotMounted), ERROR_NO_DEVICE);
        assert_eq!(map_vfs_error(VfsError::NotFound), ERROR_NO_ENTRY);
        assert_eq!(map_vfs_error(VfsError::NameTooLong), ERROR_NAME_TOO_LONG);
        assert_eq!(map_vfs_error(VfsError::NotAFile), ERROR_NOT_DIRECTORY);
        assert_eq!(map_vfs_error(VfsError::OutOfHandles), ERROR_TOO_MANY_FILES);
        assert_eq!(map_vfs_error(VfsError::BadHandle), ERROR_BAD_FD);
    }

    #[test]
    fn open_and_close_reject_untrusted_pointers_and_descriptors() {
        let mut open = valid_frame(SYS_OPEN);
        open.rdi = crate::address_space::USER_BASE - 1;
        open.rsi = 5;
        assert_eq!(systemes_syscall_dispatch(&mut open), ERROR_BAD_ADDRESS);

        let mut close = valid_frame(SYS_CLOSE);
        close.rdi = 2;
        assert_eq!(systemes_syscall_dispatch(&mut close), ERROR_BAD_FD);

        let mut file_read = valid_frame(SYS_READ);
        file_read.rdi = crate::file_descriptors::FIRST_FILE_DESCRIPTOR as u64;
        file_read.rsi = crate::address_space::USER_BASE - 1;
        file_read.rdx = 5;
        assert_eq!(systemes_syscall_dispatch(&mut file_read), ERROR_BAD_ADDRESS);
    }

    #[test]
    fn uptime_syscall_returns_the_monotonic_tick_count() {
        let mut frame = valid_frame(SYS_UPTIME);
        assert_eq!(systemes_syscall_dispatch(&mut frame), crate::clock::ticks());
    }

}
