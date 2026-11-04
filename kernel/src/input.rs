//! Deferred PS/2 event consumption and kernel input state.
//!
//! IRQ handlers only enqueue compact events. This module consumes them from
//! the normal kernel context, keeping decoding and state updates out of IRQs.
use core::sync::atomic::{AtomicI32, AtomicU64, AtomicU8, Ordering};

use crate::ps2::{self, KeyEvent, MouseEvent};

const KEY_WORDS: usize = 8;
const MAX_EVENTS_PER_POLL: usize = 32;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
}

pub struct InputState {
    pressed: [AtomicU64; KEY_WORDS],
    mouse_x: AtomicI32,
    mouse_y: AtomicI32,
    mouse_buttons: AtomicU8,
}

impl InputState {
    pub const fn new() -> Self {
        Self {
            pressed: [const { AtomicU64::new(0) }; KEY_WORDS],
            mouse_x: AtomicI32::new(0),
            mouse_y: AtomicI32::new(0),
            mouse_buttons: AtomicU8::new(0),
        }
    }

    fn apply_key(&self, event: KeyEvent) {
        let KeyEvent::Scancode { code, pressed, extended } = event;
        let index = code as usize + if extended { 256 } else { 0 };
        let word = index / 64;
        let mask = 1u64 << (index % 64);
        if pressed {
            self.pressed[word].fetch_or(mask, Ordering::AcqRel);
        } else {
            self.pressed[word].fetch_and(!mask, Ordering::AcqRel);
        }
    }

    fn apply_mouse(&self, event: MouseEvent) {
        let _ = self.mouse_x.fetch_update(Ordering::AcqRel, Ordering::Acquire,
            |value| Some(value.saturating_add(event.dx as i32)));
        let _ = self.mouse_y.fetch_update(Ordering::AcqRel, Ordering::Acquire,
            |value| Some(value.saturating_add(event.dy as i32)));
        self.mouse_buttons.store(event.buttons & 0x07, Ordering::Release);
    }

    pub fn key_is_pressed(&self, code: u8, extended: bool) -> bool {
        let index = code as usize + if extended { 256 } else { 0 };
        let word = index / 64;
        let mask = 1u64 << (index % 64);
        self.pressed[word].load(Ordering::Acquire) & mask != 0
    }

    pub fn mouse_state(&self) -> MouseState {
        MouseState {
            x: self.mouse_x.load(Ordering::Acquire),
            y: self.mouse_y.load(Ordering::Acquire),
            buttons: self.mouse_buttons.load(Ordering::Acquire),
        }
    }
}

static INPUT_STATE: InputState = InputState::new();

/// Consume at most 32 queued events without doing work inside an IRQ handler.
pub fn poll_events() -> usize {
    let mut processed = 0;
    while processed < MAX_EVENTS_PER_POLL {
        let mut found = false;
        if let Some(event) = ps2::pop_key_event() {
            INPUT_STATE.apply_key(event);
            processed += 1;
            found = true;
        }
        if processed < MAX_EVENTS_PER_POLL {
            if let Some(event) = ps2::pop_mouse_event() {
                INPUT_STATE.apply_mouse(event);
                processed += 1;
                found = true;
            }
        }
        if !found {
            break;
        }
    }
    processed
}

pub fn key_is_pressed(code: u8, extended: bool) -> bool {
    INPUT_STATE.key_is_pressed(code, extended)
}

pub fn mouse_state() -> MouseState {
    INPUT_STATE.mouse_state()
}

/// Idle loop: process deferred input, then sleep until the next interrupt.
pub fn run_event_loop() -> ! {
    loop {
        if poll_events() == 0 {
            // STI's one-instruction interrupt shadow makes STI; HLT an atomic
            // idle sequence: an IRQ arriving between polling and HLT wakes us.
            unsafe { core::arch::asm!("sti", "hlt", options(nomem, nostack)); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_state_tracks_make_break_and_extended_keys_separately() {
        let state = InputState::new();
        let ordinary = KeyEvent::Scancode { code: 0x48, pressed: true, extended: false };
        let extended = KeyEvent::Scancode { code: 0x48, pressed: true, extended: true };
        state.apply_key(ordinary);
        state.apply_key(extended);
        assert!(state.key_is_pressed(0x48, false));
        assert!(state.key_is_pressed(0x48, true));
        state.apply_key(KeyEvent::Scancode { code: 0x48, pressed: false, extended: true });
        assert!(state.key_is_pressed(0x48, false));
        assert!(!state.key_is_pressed(0x48, true));
    }

    #[test]
    fn mouse_state_accumulates_signed_motion_and_tracks_buttons() {
        let state = InputState::new();
        state.apply_mouse(MouseEvent { dx: -12, dy: 7, buttons: 0b101 });
        state.apply_mouse(MouseEvent { dx: 5, dy: -3, buttons: 0b010 });
        assert_eq!(state.mouse_state(), MouseState { x: -7, y: 4, buttons: 0b010 });
    }

    #[test]
    fn mouse_coordinates_saturate_instead_of_wrapping() {
        let state = InputState::new();
        state.mouse_x.store(i32::MAX - 1, Ordering::Release);
        state.apply_mouse(MouseEvent { dx: 10, dy: 0, buttons: 0 });
        assert_eq!(state.mouse_state().x, i32::MAX);
    }
}
