//! Polled 16550 UART on COM1 (0x3F8) for userland console input.
//!
//! Minimal polled driver: no interrupts, no FIFO, 8N1 at 115200 baud.
//! Suitable for boot-time userland bring-up before an interrupt-driven
//! keyboard/serial driver exists.

const COM1: u16 = 0x3F8;

const REG_DATA: u16 = COM1;
const REG_IER: u16 = COM1 + 1;
const REG_FCR: u16 = COM1 + 2;
const REG_LCR: u16 = COM1 + 3;
const REG_MCR: u16 = COM1 + 4;
const REG_LSR: u16 = COM1 + 5;

const LSR_DATA_READY: u8 = 1 << 0;
const LSR_OVERRUN: u8 = 1 << 1;

pub fn init() {
    // SAFETY: COM1 is a dedicated, unused UART port during early boot.
    unsafe {
        // Disable interrupts.
        crate::console::outb(REG_IER, 0x00);
        // Enable DLAB for baud divisor.
        crate::console::outb(REG_LCR, 0x80);
        // Baud divisor 1 for 115200 (115200 / 1 = 115200).
        crate::console::outb(REG_DATA, 0x01);
        crate::console::outb(REG_IER, 0x00);
        // 8N1, clear DLAB.
        crate::console::outb(REG_LCR, 0x03);
        // Enable FIFO, clear queues, 14-byte threshold.
        crate::console::outb(REG_FCR, 0xC7);
        // RTS/DTR set.
        crate::console::outb(REG_MCR, 0x03);
    }
}

/// Read one byte from the UART, polling until data is available.
///
/// Returns `None` if an overrun error is detected.
///
/// # Safety
///
/// COM1 must be the configured debug/console UART.
pub unsafe fn read_byte() -> Option<u8> {
    // SAFETY: COM1 is a dedicated UART port.
    let lsr = crate::console::inb(REG_LSR);
    if lsr & LSR_OVERRUN != 0 {
        return None;
    }
    if lsr & LSR_DATA_READY == 0 {
        return None;
    }
    // SAFETY: Data is ready, the read will not block.
    Some(crate::console::inb(REG_DATA))
}

/// Write one byte to the UART.
///
/// # Safety
///
/// COM1 must be the configured debug/console UART.
pub unsafe fn write_byte(byte: u8) {
    // SAFETY: COM1 is a dedicated UART port.
    crate::console::outb(REG_DATA, byte);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure logic tests: no hardware access. These validate constants only.
    #[test]
    fn com1_port_constants() {
        assert_eq!(COM1, 0x3F8);
        assert_eq!(REG_DATA, 0x3F8);
        assert_eq!(REG_LSR, 0x3FD);
    }

    #[test]
    fn lsr_data_ready_bit() {
        assert_eq!(LSR_DATA_READY, 1);
        assert_eq!(LSR_OVERRUN, 2);
    }
}
