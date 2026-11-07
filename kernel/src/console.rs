//! Shared diagnostic output for the QEMU debug port and a detected COM1 UART.
//!
//! Formatting is pure and host-tested. Port I/O is only used from ring 0.

use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};

pub const DEBUG_PORT: u16 = 0xE9;
const COM1_BASE: u16 = 0x3F8;
static SERIAL_INITIALIZED: AtomicBool = AtomicBool::new(false);

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

unsafe fn inb(port: u16) -> u8 {
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

/// Initialize COM1 using a loopback check to avoid reporting a missing UART.
///
/// # Safety
///
/// The caller must be running at ring 0 and must have exclusive access to COM1.
pub unsafe fn initialize_serial() -> bool {
    // SAFETY: COM1 is the conventional PC serial port and boot runs single-core.
    unsafe {
        outb(COM1_BASE + 1, 0x00);
        outb(COM1_BASE + 3, 0x80);
        outb(COM1_BASE, 0x01);
        outb(COM1_BASE + 1, 0x00);
        outb(COM1_BASE + 3, 0x03);
        outb(COM1_BASE + 2, 0xC7);
        outb(COM1_BASE + 4, 0x1E);
        outb(COM1_BASE, 0xAE);
    }

    // SAFETY: The UART data register is readable while loopback mode is set.
    if unsafe { inb(COM1_BASE) } != 0xAE {
        // SAFETY: Restore the modem-control register after the failed probe.
        unsafe { outb(COM1_BASE + 4, 0x00) };
        return false;
    }

    // SAFETY: Leave loopback mode and enable the standard DTR/RTS/OUT2 signals.
    unsafe { outb(COM1_BASE + 4, 0x0B) };
    SERIAL_INITIALIZED.store(true, Ordering::Release);
    true
}

unsafe fn serial_write_byte(byte: u8) {
    if !SERIAL_INITIALIZED.load(Ordering::Acquire) {
        return;
    }

    for _ in 0..100_000 {
        // SAFETY: COM1 was initialized and detected by initialize_serial.
        if unsafe { inb(COM1_BASE + 5) } & 0x20 != 0 {
            // SAFETY: The transmitter is ready.
            unsafe { outb(COM1_BASE, byte) };
            break;
        }
        core::hint::spin_loop();
    }
}

/// Write raw bytes to all available diagnostic consoles.
///
/// # Safety
///
/// The caller must be running at ring 0; port 0xE9 and COM1 must be safe to access.
pub unsafe fn debug_write(bytes: &[u8]) {
    for byte in bytes {
        // SAFETY: The caller guarantees the diagnostic ports are available.
        unsafe { outb(DEBUG_PORT, *byte) };
        // SAFETY: Writes are skipped unless COM1 passed its loopback probe.
        unsafe { serial_write_byte(*byte) };
    }
}

struct DiagnosticWriter;

impl Write for DiagnosticWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        // SAFETY: Formatting is only used in ring-0 diagnostics.
        unsafe { debug_write(text.as_bytes()) };
        Ok(())
    }
}

/// Format a diagnostic message to all available consoles.
///
/// # Safety
///
/// The caller must be running at ring 0 and must not invoke this concurrently
/// with another CPU writing the same UART.
pub unsafe fn debug_fmt(arguments: fmt::Arguments<'_>) {
    let _ = DiagnosticWriter.write_fmt(arguments);
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
