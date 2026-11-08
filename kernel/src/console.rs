//! Shared diagnostic console on the QEMU debug port.
//!
//! Formatting is pure and host-tested. The output path performs x86 port I/O
//! and is only valid while the QEMU debug console is configured at port 0xE9.

pub const DEBUG_PORT: u16 = 0xE9;

/// Encode `value` as exactly 16 uppercase hexadecimal digits.
pub fn encode_hex(value: u64, out: &mut [u8; 16]) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for (index, slot) in out.iter_mut().enumerate() {
        let shift = (15 - index) * 4;
        *slot = HEX[((value >> shift) & 0xF) as usize];
    }
}

/// Encode `value` as decimal digits without leading zeros.
///
/// Returns the number of bytes written at the start of `out`. A 20-byte
/// buffer always accepts every `u64` value.
pub fn encode_decimal(value: u64, out: &mut [u8; 20]) -> usize {
    if value == 0 {
        out[0] = b'0';
        return 1;
    }

    let mut index = out.len();
    let mut remaining = value;
    while remaining != 0 {
        index -= 1;
        out[index] = b'0' + (remaining % 10) as u8;
        remaining /= 10;
    }

    let length = out.len() - index;
    out.copy_within(index.., 0);
    length
}

/// Write one byte to an x86 I/O port.
///
/// # Safety
///
/// The caller must pass a port that is valid for this CPU and must ensure the
/// access does not conflict with a device owned by another context.
pub unsafe fn outb(port: u16, value: u8) {
    // SAFETY: The caller guarantees the port is valid for this CPU.
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        )
    };
}

/// Read one byte from an x86 I/O port.
///
/// # Safety
///
/// The caller must pass a port that is valid for this CPU and must ensure the
/// access does not conflict with a device owned by another context.
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: The caller guarantees the port is valid for this CPU.
    unsafe {
        core::arch::asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        )
    };
    value
}

/// Write raw bytes to the QEMU debug console.
///
/// # Safety
///
/// Port 0xE9 must be the configured debug console for the current boot.
pub unsafe fn debug_write(bytes: &[u8]) {
    for byte in bytes {
        // SAFETY: The caller guarantees the debug console is available.
        unsafe { outb(DEBUG_PORT, *byte) };
    }
}

/// Write `value` as 16 uppercase hexadecimal digits.
///
/// # Safety
///
/// Port 0xE9 must be the configured debug console for the current boot.
pub unsafe fn debug_hex(value: u64) {
    let mut digits = [0u8; 16];
    encode_hex(value, &mut digits);
    // SAFETY: The caller guarantees the debug console is available.
    unsafe { debug_write(&digits) };
}

/// Write `value` as decimal digits without leading zeros.
///
/// # Safety
///
/// Port 0xE9 must be the configured debug console for the current boot.
pub unsafe fn debug_dec(value: u64) {
    let mut digits = [0u8; 20];
    let length = encode_decimal(value, &mut digits);
    // SAFETY: `length` is at most `digits.len()` by construction.
    unsafe { debug_write(&digits[..length]) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_zero_as_sixteen_hex_digits() {
        let mut digits = [0u8; 16];
        encode_hex(0, &mut digits);
        assert_eq!(&digits, b"0000000000000000");
    }

    #[test]
    fn encodes_hex_without_lowercase_or_truncation() {
        let mut digits = [0u8; 16];
        encode_hex(0x0123_4567_89AB_CDEF, &mut digits);
        assert_eq!(&digits, b"0123456789ABCDEF");

        encode_hex(u64::MAX, &mut digits);
        assert_eq!(&digits, b"FFFFFFFFFFFFFFFF");
    }

    #[test]
    fn encodes_zero_and_small_decimals() {
        let mut digits = [0u8; 20];
        assert_eq!(encode_decimal(0, &mut digits), 1);
        assert_eq!(&digits[..1], b"0");

        assert_eq!(encode_decimal(9, &mut digits), 1);
        assert_eq!(&digits[..1], b"9");

        assert_eq!(encode_decimal(10, &mut digits), 2);
        assert_eq!(&digits[..2], b"10");

        assert_eq!(encode_decimal(1_000_000, &mut digits), 7);
        assert_eq!(&digits[..7], b"1000000");
    }

    #[test]
    fn encodes_u64_max_with_all_twenty_digits() {
        let mut digits = [0u8; 20];
        assert_eq!(encode_decimal(u64::MAX, &mut digits), 20);
        assert_eq!(&digits, b"18446744073709551615");
    }

    #[test]
    fn decimal_encoding_has_no_leading_zeros() {
        let mut digits = [0u8; 20];
        let length = encode_decimal(42, &mut digits);
        assert_eq!(length, 2);
        assert_eq!(&digits[..length], b"42");
    }
}
