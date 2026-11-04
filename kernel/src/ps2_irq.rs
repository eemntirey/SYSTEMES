//! Minimal PS/2 IRQ data path. Handlers only read one controller byte,
//! update lock-free decoder state, and enqueue compact events.
use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use crate::ps2::{self, KeyEvent, MouseEvent};

const STATUS_PORT: u16 = 0x64;
const DATA_PORT: u16 = 0x60;
const OUTPUT_FULL: u8 = 1;
const AUXILIARY_DATA: u8 = 1 << 5;

static KEY_EXTENDED: AtomicBool = AtomicBool::new(false);
static MOUSE_PACKET: [AtomicU8; 3] = [const { AtomicU8::new(0) }; 3];
static MOUSE_INDEX: AtomicU8 = AtomicU8::new(0);
static KEY_DROPPED: AtomicU8 = AtomicU8::new(0);
static MOUSE_DROPPED: AtomicU8 = AtomicU8::new(0);

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: Caller runs in the ring-0 PS/2 interrupt path.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags)); }
    value
}

fn push_key(byte: u8) {
    if byte == 0xE0 {
        KEY_EXTENDED.store(true, Ordering::Release);
        return;
    }
    let extended = KEY_EXTENDED.swap(false, Ordering::AcqRel);
    let event = KeyEvent::Scancode {
        code: byte & 0x7F,
        pressed: byte & 0x80 == 0,
        extended,
    };
    if ps2::push_key_event(event).is_err() {
        KEY_DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}

fn push_mouse(byte: u8) {
    let mut index = MOUSE_INDEX.load(Ordering::Relaxed);
    if index == 0 && byte & 0x08 == 0 {
        return;
    }
    if index >= 3 { index = 0; }
    MOUSE_PACKET[index as usize].store(byte, Ordering::Relaxed);
    index += 1;
    if index < 3 {
        MOUSE_INDEX.store(index, Ordering::Release);
        return;
    }
    MOUSE_INDEX.store(0, Ordering::Release);
    let flags = MOUSE_PACKET[0].load(Ordering::Relaxed);
    if flags & 0xC0 != 0 { return; }
    let x = MOUSE_PACKET[1].load(Ordering::Relaxed) as i16;
    let y = MOUSE_PACKET[2].load(Ordering::Relaxed) as i16;
    let dx = if flags & 0x10 != 0 { x - 256 } else { x };
    let raw_y = if flags & 0x20 != 0 { y - 256 } else { y };
    if ps2::push_mouse_event(MouseEvent { dx, dy: -raw_y, buttons: flags & 0x07 }).is_err() {
        MOUSE_DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}

/// Consume at most one byte; the controller status bit identifies the source.
pub fn handle_irq(irq: u8) {
    if irq != 1 && irq != 12 { return; }
    // SAFETY: Port I/O is valid in this ring-0 interrupt handler.
    unsafe {
        let status = inb(STATUS_PORT);
        if status & OUTPUT_FULL == 0 { return; }
        let byte = inb(DATA_PORT);
        if status & AUXILIARY_DATA != 0 {
            push_mouse(byte);
        } else {
            push_key(byte);
        }
    }
}

pub fn dropped_events() -> (u8, u8) {
    (KEY_DROPPED.load(Ordering::Acquire), MOUSE_DROPPED.load(Ordering::Acquire))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_prefix_state_is_consumed_once_and_queues_extended_make() {
        while ps2::pop_key_event().is_some() {}
        KEY_EXTENDED.store(false, Ordering::Release);
        push_key(0xE0);
        assert!(KEY_EXTENDED.load(Ordering::Acquire));
        push_key(0x48);
        assert!(!KEY_EXTENDED.load(Ordering::Acquire));
        assert_eq!(ps2::pop_key_event(), Some(KeyEvent::Scancode {
            code: 0x48, pressed: true, extended: true
        }));
    }

    #[test]
    fn mouse_packet_state_resets_and_queues_signed_motion() {
        while ps2::pop_mouse_event().is_some() {}
        MOUSE_INDEX.store(0, Ordering::Release);
        push_mouse(0x19);
        push_mouse(0xFE);
        push_mouse(0x03);
        assert_eq!(MOUSE_INDEX.load(Ordering::Acquire), 0);
        assert_eq!(ps2::pop_mouse_event(), Some(MouseEvent { dx: -2, dy: -3, buttons: 1 }));
    }

    #[test]
    fn mouse_overflow_packet_is_discarded() {
        while ps2::pop_mouse_event().is_some() {}
        MOUSE_INDEX.store(0, Ordering::Release);
        push_mouse(0xC9);
        push_mouse(0x7F);
        push_mouse(0x7F);
        assert_eq!(ps2::pop_mouse_event(), None);
    }
}
