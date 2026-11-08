//! Small Ring 3 console program that prompts, reads one UART byte, and echoes it.

use crate::address_space::USER_BASE;

const SHELL_CODE: &[u8] = &[
    0xB8, 1, 0, 0, 0, 0xBF, 1, 0, 0, 0, 0x48, 0xBE, 0, 0x70, 0, 0, 0x80, 0, 0, 0, 0xBA, 2,
    0, 0, 0, 0x0F, 0x05, 0xB8, 2, 0, 0, 0, 0xBF, 1, 0, 0, 0, 0x48, 0xBE, 0, 0x70, 0, 0, 0x80,
    0, 0, 0, 0xBA, 1, 0, 0, 0, 0x0F, 0x05, 0xB8, 1, 0, 0, 0, 0xBF, 1, 0, 0, 0, 0x48, 0xBE,
    0, 0x70, 0, 0, 0x80, 0, 0, 0, 0xBA, 1, 0, 0, 0, 0x0F, 0x05, 0xEB, 0xAD,
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
    put_u64(&mut elf, 104, 4096);
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
    use crate::elf_loader::{ElfImage, ElfError};

    #[test]
    fn shell_elf_is_a_valid_executable_image() {
        let image = ElfImage::parse(SHELL_ELF).expect("shell ELF should parse");
        assert_eq!(image.entry, USER_BASE + 0x6000);
        assert_eq!(image.segment_count, 1);
        assert_eq!(image.segments[0].flags, 5);
    }

    #[test]
    fn shell_code_uses_read_and_write_syscalls() {
        assert_eq!(SHELL_CODE.windows(2).filter(|bytes| *bytes == [0x0F, 0x05]).count(), 3);
    }

    #[test]
    fn shell_has_no_writable_executable_segment() {
        let image = ElfImage::parse(SHELL_ELF).expect("shell ELF should parse");
        assert_ne!(image.segments[0].flags & 3, 3);
        assert_eq!(
            ElfImage::parse(&SHELL_ELF[..127]),
            Err(ElfError::SegmentOutsideImage)
        );
    }
}
