//! Polled COM1 UART I/O used by the interactive kernel shell.

use crate::console::outb;

const COM1: u16 = 0x3F8;
const DATA: u16 = COM1;
const INTERRUPT_ENABLE: u16 = COM1 + 1;
const FIFO_CONTROL: u16 = COM1 + 2;
const LINE_CONTROL: u16 = COM1 + 3;
const MODEM_CONTROL: u16 = COM1 + 4;
const LINE_STATUS: u16 = COM1 + 5;
const TRANSMIT_EMPTY: u8 = 1 << 5;
const DATA_READY: u8 = 1;
const POLL_LIMIT: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SerialError {
    TransmitTimeout,
}

/// Configure COM1 for 115200 baud, 8 data bits, no parity, one stop bit.
pub unsafe fn initialize() {
    // SAFETY: COM1 is the conventional PC UART used by the boot console.
    unsafe {
        outb(INTERRUPT_ENABLE, 0x00);
        outb(LINE_CONTROL, 0x80);
        outb(DATA, 0x01);
        outb(INTERRUPT_ENABLE, 0x00);
        outb(LINE_CONTROL, 0x03);
        outb(FIFO_CONTROL, 0xC7);
        outb(MODEM_CONTROL, 0x0B);
    }
}

pub fn try_read_byte() -> Option<u8> {
    // SAFETY: Reads the COM1 line-status register.
    if unsafe { crate::console::inb(LINE_STATUS) } & DATA_READY == 0 {
        return None;
    }
    // SAFETY: The data-ready status bit guarantees a byte is available.
    Some(unsafe { crate::console::inb(DATA) })
}

pub fn write_byte(byte: u8) -> Result<(), SerialError> {
    for _ in 0..POLL_LIMIT {
        // SAFETY: Reads the COM1 line-status register.
        if unsafe { crate::console::inb(LINE_STATUS) } & TRANSMIT_EMPTY != 0 {
            // SAFETY: COM1 transmit holding register is ready.
            unsafe { outb(DATA, byte) };
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(SerialError::TransmitTimeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uart_registers_use_the_standard_com1_offsets() {
        assert_eq!(DATA, 0x3F8);
        assert_eq!(INTERRUPT_ENABLE, 0x3F9);
        assert_eq!(LINE_STATUS, 0x3FD);
        assert_eq!(TRANSMIT_EMPTY, 0x20);
    }
}