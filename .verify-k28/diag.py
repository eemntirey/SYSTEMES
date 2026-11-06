from pathlib import Path

p = Path('kernel/src/main.rs')
t = p.read_text(encoding='utf-8')

helper = r'''    pub fn debug_hex(label: &[u8], value: usize) {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let mut buffer = [b'0'; 18];
        buffer[0] = b'0';
        buffer[1] = b'x';
        let mut index = 0;
        while index < 16 {
            let shift = 60 - index * 4;
            buffer[2 + index] = HEX[(value >> shift) & 0xF];
            index += 1;
        }
        unsafe {
            super::debug_write(label);
            super::debug_write(&buffer);
            super::debug_write(b"\r\n");
        }
    }

'''
anchor = '    pub fn verified_counts() -> [usize; 3] {'
assert t.count(anchor) == 1
t = t.replace(anchor, helper + anchor, 1)

old = r'''        if verified.iter().any(|count| *count < fpu_context_test::MIN_VERIFIED) {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: FPU task verification counter too low\r\n"); }
            halt();
        }'''
new = r'''        unsafe {
            fpu_context_test::debug_hex(b"SYSTEMES_TEST: tick=", systemes_kernel::interrupts::local_timer_ticks() as usize);
            fpu_context_test::debug_hex(b"SYSTEMES_TEST: A=", verified[0]);
            fpu_context_test::debug_hex(b"SYSTEMES_TEST: B=", verified[1]);
            fpu_context_test::debug_hex(b"SYSTEMES_TEST: C=", verified[2]);
        }
        if verified.iter().any(|count| *count < fpu_context_test::MIN_VERIFIED) {
            unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: FPU task verification counter too low\r\n"); }
            halt();
        }'''
assert t.count(old) == 1, t.count(old)
t = t.replace(old, new, 1)

p.write_text(t, encoding='utf-8')
print('patched')
