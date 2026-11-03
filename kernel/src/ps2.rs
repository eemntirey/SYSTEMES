//! PS/2 input protocol decoding and bounded event queues.
//!
//! Hardware command sequencing and interrupt routing are deliberately kept in
//! interrupts.rs until the controller and IOAPIC routes can be enabled atomically.

use core::sync::atomic::{AtomicUsize, Ordering};

const QUEUE_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyEvent {
    Scancode { code: u8, pressed: bool, extended: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MouseEvent {
    pub dx: i16,
    pub dy: i16,
    pub buttons: u8,
}

struct SpscQueue<T: Copy, const N: usize> {
    slots: [core::cell::UnsafeCell<Option<T>>; N],
    head: AtomicUsize,
    tail: AtomicUsize,
}

unsafe impl<T: Copy + Send, const N: usize> Sync for SpscQueue<T, N> {}

impl<T: Copy, const N: usize> SpscQueue<T, N> {
    const fn new() -> Self {
        Self {
            slots: [const { core::cell::UnsafeCell::new(None) }; N],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn push(&self, value: T) -> Result<(), T> {
        let head = self.head.load(Ordering::Relaxed);
        let next = (head + 1) % N;
        if next == self.tail.load(Ordering::Acquire) {
            return Err(value);
        }
        // SAFETY: The producer exclusively owns the slot until publishing head.
        unsafe { *self.slots[head].get() = Some(value); }
        self.head.store(next, Ordering::Release);
        Ok(())
    }

    fn pop(&self) -> Option<T> {
        let tail = self.tail.load(Ordering::Relaxed);
        if tail == self.head.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: The consumer exclusively owns the slot until publishing tail.
        let value = unsafe { (*self.slots[tail].get()).take() };
        self.tail.store((tail + 1) % N, Ordering::Release);
        value
    }
}

static KEY_EVENTS: SpscQueue<KeyEvent, QUEUE_CAPACITY> = SpscQueue::new();
static MOUSE_EVENTS: SpscQueue<MouseEvent, QUEUE_CAPACITY> = SpscQueue::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueError { Full }

pub fn push_key_event(event: KeyEvent) -> Result<(), QueueError> {
    KEY_EVENTS.push(event).map_err(|_| QueueError::Full)
}
pub fn pop_key_event() -> Option<KeyEvent> { KEY_EVENTS.pop() }

pub fn push_mouse_event(event: MouseEvent) -> Result<(), QueueError> {
    MOUSE_EVENTS.push(event).map_err(|_| QueueError::Full)
}
pub fn pop_mouse_event() -> Option<MouseEvent> { MOUSE_EVENTS.pop() }

#[derive(Clone, Copy, Debug, Default)]
pub struct KeyboardDecoder {
    extended: bool,
    release: bool,
}

impl KeyboardDecoder {
    /// Decode Set-1 make/break bytes. Prefix state is retained across IRQs.
    pub fn feed(&mut self, byte: u8) -> Option<KeyEvent> {
        match byte {
            0xE0 => { self.extended = true; None }
            0xF0 => { self.release = true; None }
            value => {
                let event = KeyEvent::Scancode {
                    code: value & 0x7F,
                    pressed: !self.release && value & 0x80 == 0,
                    extended: self.extended,
                };
                self.extended = false;
                self.release = false;
                Some(event)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MouseDecoder {
    packet: [u8; 3],
    index: u8,
}

impl MouseDecoder {
    /// Decode the standard three-byte PS/2 packet (no wheel extension).
    pub fn feed(&mut self, byte: u8) -> Option<MouseEvent> {
        if self.index == 0 && byte & 0x08 == 0 {
            return None; // Packet synchronization: first byte always has bit 3 set.
        }
        self.packet[self.index as usize] = byte;
        self.index += 1;
        if self.index != 3 { return None; }
        self.index = 0;
        let flags = self.packet[0];
        let dx = if flags & 0x10 != 0 { (self.packet[1] as i16) - 256 } else { self.packet[1] as i16 };
        let raw_y = if flags & 0x20 != 0 { (self.packet[2] as i16) - 256 } else { self.packet[2] as i16 };
        Some(MouseEvent { dx, dy: -raw_y, buttons: flags & 0x07 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_decoder_tracks_extended_make_and_break_sequences() {
        let mut decoder = KeyboardDecoder::default();
        assert_eq!(decoder.feed(0xE0), None);
        assert_eq!(decoder.feed(0x75), Some(KeyEvent::Scancode { code: 0x75, pressed: true, extended: true }));
        assert_eq!(decoder.feed(0xF0), None);
        assert_eq!(decoder.feed(0x1C), Some(KeyEvent::Scancode { code: 0x1C, pressed: false, extended: false }));
    }

    #[test]
    fn keyboard_decoder_handles_set_one_break_bytes() {
        let mut decoder = KeyboardDecoder::default();
        assert_eq!(decoder.feed(0x9E), Some(KeyEvent::Scancode { code: 0x1E, pressed: false, extended: false }));
    }

    #[test]
    fn mouse_decoder_synchronizes_and_decodes_signed_motion() {
        let mut decoder = MouseDecoder::default();
        assert_eq!(decoder.feed(0x01), None);
        assert_eq!(decoder.feed(0x19), None);
        assert_eq!(decoder.feed(0xFE), None);
        assert_eq!(decoder.feed(0x03), Some(MouseEvent { dx: -2, dy: -3, buttons: 1 }));
    }

    #[test]
    fn mouse_decoder_decodes_positive_motion_and_buttons() {
        let mut decoder = MouseDecoder::default();
        decoder.feed(0x09);
        decoder.feed(12);
        assert_eq!(decoder.feed(7), Some(MouseEvent { dx: 12, dy: -7, buttons: 1 }));
    }

    #[test]
    fn event_queue_preserves_fifo_order() {
        let queue: SpscQueue<u8, 4> = SpscQueue::new();
        assert_eq!(queue.push(1), Ok(()));
        assert_eq!(queue.push(2), Ok(()));
        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn event_queue_reports_full_without_overwriting_unread_events() {
        let queue: SpscQueue<u8, 3> = SpscQueue::new();
        assert_eq!(queue.push(1), Ok(()));
        assert_eq!(queue.push(2), Ok(()));
        assert_eq!(queue.push(3), Err(3));
        assert_eq!(queue.pop(), Some(1));
    }
}
