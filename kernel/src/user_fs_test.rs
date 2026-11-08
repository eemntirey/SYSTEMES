//! Feature-gated Ring 3 integration test for SYS_OPEN/SYS_READ/SYS_CLOSE.

use super::*;

pub fn run(
    allocator: &mut PhysicalFrameAllocator,
    kernel_root: u64,
) -> Result<(), &'static [u8]> {
    const CODE_ADDRESS: u64 = USER_BASE + 0x80_0000;
    const STACK_ADDRESS: u64 = USER_BASE + 0x80_1000;

    fn emit(code: &mut [u8], length: &mut usize, bytes: &[u8]) {
        let end = *length + bytes.len();
        code[*length..end].copy_from_slice(bytes);
        *length = end;
    }

    fn emit_imm32(code: &mut [u8], length: &mut usize, opcode: u8, value: u32) {
        emit(code, length, &[opcode]);
        emit(code, length, &value.to_le_bytes());
    }

    fn emit_lea_rdi(code: &mut [u8], length: &mut usize) -> usize {
        let offset = *length;
        emit(code, length, &[0x48, 0x8D, 0x3D, 0, 0, 0, 0]);
        offset
    }

    fn emit_read_buffer(code: &mut [u8], length: &mut usize) {
        emit(code, length, &[0x48, 0x8D, 0x74, 0x24, 0xC0]); // lea rsi,[rsp-64]
    }

    fn emit_store_rax(code: &mut [u8], length: &mut usize, displacement: u8) {
        emit(code, length, &[0x48, 0x89, 0x44, 0x24, displacement]);
    }

    let free_pages_before = allocator.free_pages();
    let code_frame = allocator.allocate_frame()
        .ok_or(b"SYSTEMES_KERNEL_FAIL: user FS code frame unavailable\r\n" as &'static [u8])?;
    let stack_frame = allocator.allocate_frame()
        .ok_or(b"SYSTEMES_KERNEL_FAIL: user FS stack frame unavailable\r\n" as &'static [u8])?;
    let page_size = systemes_kernel::memory::PAGE_SIZE;
    if code_frame % page_size != 0 || stack_frame % page_size != 0
        || code_frame >= IDENTITY_MAP_BYTES || stack_frame >= IDENTITY_MAP_BYTES
    {
        return Err(b"SYSTEMES_KERNEL_FAIL: user FS test frame invalid\r\n");
    }

    let mut code = [0u8; 128];
    let mut code_len = 0usize;

    emit_imm32(&mut code, &mut code_len, 0xB8, systemes_kernel::syscall::SYS_OPEN as u32);
    let hello_lea = emit_lea_rdi(&mut code, &mut code_len);
    emit_imm32(&mut code, &mut code_len, 0xBE, 10);
    emit(&mut code, &mut code_len, &[0x0F, 0x05]); // open /hello.txt
    emit(&mut code, &mut code_len, &[0x49, 0x89, 0xC4]); // mov r12,rax

    emit_imm32(&mut code, &mut code_len, 0xB8, systemes_kernel::syscall::SYS_READ as u32);
    emit(&mut code, &mut code_len, &[0x4C, 0x89, 0xE7]); // mov rdi,r12
    emit_read_buffer(&mut code, &mut code_len);
    emit_imm32(&mut code, &mut code_len, 0xBA, 5);
    emit(&mut code, &mut code_len, &[0x0F, 0x05]);
    emit_store_rax(&mut code, &mut code_len, 0xF8); // [rsp-8] = read count

    emit_imm32(&mut code, &mut code_len, 0xB8, systemes_kernel::syscall::SYS_READ as u32);
    emit(&mut code, &mut code_len, &[0x4C, 0x89, 0xE7]);
    emit_read_buffer(&mut code, &mut code_len);
    emit_imm32(&mut code, &mut code_len, 0xBA, 1);
    emit(&mut code, &mut code_len, &[0x0F, 0x05]);
    emit_store_rax(&mut code, &mut code_len, 0xE0); // [rsp-32] = EOF count

    emit_imm32(&mut code, &mut code_len, 0xB8, systemes_kernel::syscall::SYS_CLOSE as u32);
    emit(&mut code, &mut code_len, &[0x4C, 0x89, 0xE7]);
    emit(&mut code, &mut code_len, &[0x0F, 0x05]);
    emit_store_rax(&mut code, &mut code_len, 0xF0); // [rsp-16] = close result

    emit_imm32(&mut code, &mut code_len, 0xB8, systemes_kernel::syscall::SYS_OPEN as u32);
    let absent_lea = emit_lea_rdi(&mut code, &mut code_len);
    emit_imm32(&mut code, &mut code_len, 0xBE, 7);
    emit(&mut code, &mut code_len, &[0x0F, 0x05]);
    emit_store_rax(&mut code, &mut code_len, 0xE8); // [rsp-24] = ENOENT result

    emit_imm32(&mut code, &mut code_len, 0xB8, systemes_kernel::syscall::SYS_EXIT as u32);
    emit(&mut code, &mut code_len, &[0x31, 0xFF, 0x0F, 0x05, 0x0F, 0x0B]);

    let hello_path_offset = code_len;
    emit(&mut code, &mut code_len, b"/hello.txt");
    let absent_path_offset = code_len;
    emit(&mut code, &mut code_len, b"/absent");
    let hello_disp = (hello_path_offset as i32) - (hello_lea as i32 + 7);
    let absent_disp = (absent_path_offset as i32) - (absent_lea as i32 + 7);
    code[hello_lea + 3..hello_lea + 7].copy_from_slice(&hello_disp.to_le_bytes());
    code[absent_lea + 3..absent_lea + 7].copy_from_slice(&absent_disp.to_le_bytes());

    // SAFETY: These are fresh identity-mapped physical frames owned by the allocator.
    unsafe {
        core::ptr::write_bytes(code_frame as *mut u8, 0, page_size as usize);
        core::ptr::write_bytes(stack_frame as *mut u8, 0, page_size as usize);
        core::ptr::copy_nonoverlapping(code.as_ptr(), code_frame as *mut u8, code_len);
    }
    // SAFETY: The allocator and kernel root are valid during this single-core boot test.
    let mut space = unsafe { AddressSpace::create(allocator, kernel_root) }
        .map_err(|_| b"SYSTEMES_KERNEL_FAIL: user FS address space creation failed\r\n" as &'static [u8])?;
    // SAFETY: Both mappings use aligned allocated frames and addresses inside the user window.
    unsafe {
        space.map_user_page(allocator, CODE_ADDRESS, code_frame, UserPagePermissions::ReadOnlyExecute)
            .map_err(|_| b"SYSTEMES_KERNEL_FAIL: user FS code mapping failed\r\n" as &'static [u8])?;
        space.map_user_page(allocator, STACK_ADDRESS, stack_frame, UserPagePermissions::ReadWriteNoExecute)
            .map_err(|_| b"SYSTEMES_KERNEL_FAIL: user FS stack mapping failed\r\n" as &'static [u8])?;
    }

    let user_rsp = STACK_ADDRESS + page_size - 16;
    let previous_root = unsafe { systemes_kernel::address_space::read_cr3() };
    let previous_flags: u64;
    // SAFETY: Save flags and switch CR3 with interrupts disabled during this single-core transition.
    unsafe {
        asm!("pushfq; pop {}", out(reg) previous_flags, options(nomem, nostack, preserves_flags));
        asm!("cli", options(nomem, nostack));
        write_cr3(space.root());
    }
    systemes_kernel::syscall::reset_process_file_table();
    let exit_status = unsafe { enter_user_mode(CODE_ADDRESS, user_rsp) };

    let (read_count, eof_count, close_result, missing_result, contents) = {
        let _guard = systemes_kernel::uaccess::UserAccessGuard::new();
        // SAFETY: The test process stack is still mapped by the active process CR3.
        let read_count = unsafe { core::ptr::read_volatile((user_rsp - 8) as *const u64) };
        let close_result = unsafe { core::ptr::read_volatile((user_rsp - 16) as *const u64) };
        let missing_result = unsafe { core::ptr::read_volatile((user_rsp - 24) as *const u64) };
        let eof_count = unsafe { core::ptr::read_volatile((user_rsp - 32) as *const u64) };
        let mut contents = [0u8; 5];
        for (index, byte) in contents.iter_mut().enumerate() {
            // SAFETY: The five-byte read buffer is inside the mapped user stack page.
            *byte = unsafe { core::ptr::read_volatile((user_rsp - 64 + index as u64) as *const u8) };
        }
        (read_count, eof_count, close_result, missing_result, contents)
    };

    // SAFETY: Restore the original kernel root before reclaiming process frames.
    unsafe { write_cr3(previous_root) };
    // SAFETY: The kernel root is active, so the process-owned frames are no longer in use.
    unsafe { space.destroy(allocator) }
        .map_err(|_| b"SYSTEMES_KERNEL_FAIL: user FS process frames could not be reclaimed\r\n" as &'static [u8])?;
    if allocator.free_pages() != free_pages_before {
        return Err(b"SYSTEMES_KERNEL_FAIL: user FS process frame leak\r\n");
    }
    if previous_flags & (1 << 9) != 0 {
        // SAFETY: The original kernel address space is restored and interrupts were previously enabled.
        unsafe { asm!("sti", options(nomem, nostack)); }
    }

    if exit_status != 0 || read_count != 5 || eof_count != 0 || close_result != 0
        || missing_result != systemes_kernel::syscall::ERROR_NO_ENTRY || contents != *b"hello"
    {
        return Err(b"SYSTEMES_KERNEL_FAIL: user SYS_OPEN/READ/CLOSE assertions failed\r\n");
    }
    unsafe { debug_write(b"SYSTEMES_TEST: user SYS_OPEN/READ/CLOSE succeeded\r\n"); }
    Ok(())
}
