//! Ring 3 userland command shell (ELF64, x86_64).
//!
//! The bytecode is assembled from `kernel/userland/shell.S`. It deliberately
//! uses only the documented syscall ABI; command parsing and line editing stay
//! in user mode.

use crate::address_space::USER_BASE;

const SHELL_CODE: &[u8] = &[
    69, 49, 228, 73, 189, 0, 112, 0, 128, 0, 0, 0, 0, 77, 141, 181,
    0, 1, 0, 0, 69, 49, 228, 72, 141, 53, 59, 1, 0, 0, 186, 2,
    0, 0, 0, 232, 36, 1, 0, 0, 184, 2, 0, 0, 0, 191, 1, 0,
    0, 0, 76, 137, 246, 186, 1, 0, 0, 0, 15, 5, 72, 133, 192, 126,
    231, 65, 15, 182, 6, 60, 10, 116, 79, 60, 13, 116, 75, 60, 8, 116,
    44, 60, 127, 116, 40, 60, 32, 114, 207, 65, 131, 252, 127, 115, 201, 67,
    136, 68, 37, 0, 65, 255, 196, 184, 1, 0, 0, 0, 191, 1, 0, 0,
    0, 76, 137, 246, 186, 1, 0, 0, 0, 15, 5, 235, 171, 69, 133, 228,
    116, 166, 65, 255, 204, 72, 141, 53, 209, 0, 0, 0, 186, 3, 0, 0,
    0, 232, 182, 0, 0, 0, 235, 144, 72, 141, 53, 188, 0, 0, 0, 186,
    2, 0, 0, 0, 232, 163, 0, 0, 0, 69, 133, 228, 15, 132, 98, 255,
    255, 255, 65, 131, 252, 4, 117, 20, 65, 129, 125, 0, 101, 120, 105, 116,
    116, 127, 65, 129, 125, 0, 104, 101, 108, 112, 116, 73, 65, 131, 252, 5,
    114, 89, 65, 129, 125, 0, 101, 99, 104, 111, 117, 79, 65, 128, 125, 4,
    32, 117, 72, 68, 137, 226, 131, 234, 5, 15, 142, 37, 255, 255, 255,
    184, 1, 0, 0, 0, 191, 1, 0, 0, 0, 73, 141, 117, 5, 15, 5,
    72, 141, 53, 85, 0, 0, 0, 186, 2, 0, 0, 0, 232, 60, 0, 0,
    0, 233, 255, 254, 255, 255, 72, 141, 53, 68, 0, 0, 0, 186, 35, 0,
    0, 0, 232, 38, 0, 0, 0, 233, 233, 254, 255, 255, 72, 141, 53, 81,
    0, 0, 0, 186, 29, 0, 0, 0, 232, 16, 0, 0, 0, 233, 211, 254,
    255, 255, 184, 3, 0, 0, 0, 49, 255, 15, 5, 15, 11, 184, 1, 0,
    0, 0, 191, 1, 0, 0, 0, 15, 5, 195, 62, 32, 13, 10, 8, 32,
    8, 67, 111, 109, 109, 97, 110, 100, 115, 58, 32, 104, 101, 108, 112, 44,
    32, 101, 99, 104, 111, 32, 60, 116, 101, 120, 116, 62, 44, 32, 101, 120,
    105, 116, 13, 10, 85, 110, 107, 110, 111, 119, 110, 32, 99, 111, 109, 109,
    97, 110, 100, 46, 32, 84, 121, 112, 101, 32, 104, 101, 108, 112, 46, 13,
    10,
];

const fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset] = value as u8;
    bytes[offset + 1] = (value >> 8) as u8;
}

const fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    let mut index = 0;
    while index < 4 {
        bytes[offset + index] = (value >> (index * 8)) as u8;
        index += 1;
    }
}

const fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    let mut index = 0;
    while index < 8 {
        bytes[offset + index] = (value >> (index * 8)) as u8;
        index += 1;
    }
}

const fn build_shell_elf() -> [u8; 128 + SHELL_CODE.len()] {
    let mut elf = [0; 128 + SHELL_CODE.len()];
    elf[0] = 0x7F;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    elf[4] = 2;
    elf[5] = 1;
    elf[6] = 1;
    put_u16(&mut elf, 16, 2);
    put_u16(&mut elf, 18, 0x3E);
    put_u32(&mut elf, 20, 1);
    put_u64(&mut elf, 24, USER_BASE + 0x6000);
    put_u64(&mut elf, 32, 64);
    put_u16(&mut elf, 52, 64);
    put_u16(&mut elf, 54, 56);
    put_u16(&mut elf, 56, 1);

    put_u32(&mut elf, 64, 1);
    put_u32(&mut elf, 68, 5);
    put_u64(&mut elf, 72, 128);
    put_u64(&mut elf, 80, USER_BASE + 0x6000);
    put_u64(&mut elf, 88, USER_BASE + 0x6000);
    put_u64(&mut elf, 96, SHELL_CODE.len() as u64);
    put_u64(&mut elf, 104, SHELL_CODE.len() as u64);
    put_u64(&mut elf, 112, 1);

    let mut index = 0;
    while index < SHELL_CODE.len() {
        elf[128 + index] = SHELL_CODE[index];
        index += 1;
    }
    elf
}

pub const SHELL_ELF: &[u8] = &build_shell_elf();

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elf_loader::{ElfError, ElfImage};

    #[test]
    fn shell_elf_is_a_valid_rx_executable_image() {
        let image = ElfImage::parse(SHELL_ELF).expect("shell ELF should parse");
        assert_eq!(image.entry, USER_BASE + 0x6000);
        assert_eq!(image.segment_count, 1);
        assert_eq!(image.segments[0].flags, 5);
        assert_eq!(image.segments[0].filesz, SHELL_CODE.len() as u64);
    }

    #[test]
    fn shell_uses_documented_read_write_and_exit_syscalls() {
        let syscall_count = SHELL_CODE
            .windows(2)
            .filter(|bytes| *bytes == [0x0F, 0x05])
            .count();
        assert_eq!(syscall_count, 5);
        assert!(SHELL_ELF.windows(4).any(|bytes| bytes == b"help"));
        assert!(SHELL_ELF.windows(4).any(|bytes| bytes == b"echo"));
        assert!(SHELL_ELF.windows(4).any(|bytes| bytes == b"exit"));
        assert!(SHELL_ELF.windows(7).any(|bytes| bytes == b"Unknown"));
    }

    #[test]
    fn shell_segment_is_not_writable_and_executable() {
        let image = ElfImage::parse(SHELL_ELF).expect("shell ELF should parse");
        assert_ne!(image.segments[0].flags & 3, 3);
        assert_eq!(
            ElfImage::parse(&SHELL_ELF[..127]),
            Err(ElfError::SegmentOutsideImage)
        );
    }
}
