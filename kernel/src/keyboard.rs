//! PS/2 scan-code set 2 decoding primitives.
//!
//! The byte decoder is host-testable and deliberately separate from the 8042
//! transport/IRQ layer. The latter is not implemented by this module yet.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    pub ascii: u8,
    pub scan_code: u8,
}

/// Stateful decoder for the common US-layout subset of scan-code set 2.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScanCodeSet2Decoder {
    extended: bool,
    releasing: bool,
    left_shift: bool,
    right_shift: bool,
    caps_lock: bool,
}

impl ScanCodeSet2Decoder {
    pub const fn new() -> Self {
        Self {
            extended: false,
            releasing: false,
            left_shift: false,
            right_shift: false,
            caps_lock: false,
        }
    }

    /// Consume one controller byte. Prefixes and key releases produce no event.
    pub fn feed(&mut self, byte: u8) -> Option<KeyEvent> {
        match byte {
            0xE0 => {
                self.extended = true;
                return None;
            }
            0xF0 => {
                self.releasing = true;
                return None;
            }
            _ => {}
        }

        let extended = core::mem::replace(&mut self.extended, false);
        let releasing = core::mem::replace(&mut self.releasing, false);
        if extended {
            // Extended navigation/modifier keys are intentionally not mapped yet.
            return None;
        }

        match byte {
            0x12 => {
                self.left_shift = !releasing;
                return None;
            }
            0x59 => {
                self.right_shift = !releasing;
                return None;
            }
            0x58 if !releasing => {
                self.caps_lock = !self.caps_lock;
                return None;
            }
            0x58 => return None,
            _ if releasing => return None,
            _ => {}
        }

        let shifted = self.left_shift || self.right_shift;
        let ascii = match byte {
            0x1C => letter(b'a', shifted, self.caps_lock),
            0x32 => letter(b'b', shifted, self.caps_lock),
            0x21 => letter(b'c', shifted, self.caps_lock),
            0x23 => letter(b'd', shifted, self.caps_lock),
            0x24 => letter(b'e', shifted, self.caps_lock),
            0x2B => letter(b'f', shifted, self.caps_lock),
            0x34 => letter(b'g', shifted, self.caps_lock),
            0x33 => letter(b'h', shifted, self.caps_lock),
            0x43 => letter(b'i', shifted, self.caps_lock),
            0x3B => letter(b'j', shifted, self.caps_lock),
            0x42 => letter(b'k', shifted, self.caps_lock),
            0x4B => letter(b'l', shifted, self.caps_lock),
            0x3A => letter(b'm', shifted, self.caps_lock),
            0x31 => letter(b'n', shifted, self.caps_lock),
            0x44 => letter(b'o', shifted, self.caps_lock),
            0x4D => letter(b'p', shifted, self.caps_lock),
            0x15 => letter(b'q', shifted, self.caps_lock),
            0x2D => letter(b'r', shifted, self.caps_lock),
            0x1B => letter(b's', shifted, self.caps_lock),
            0x2C => letter(b't', shifted, self.caps_lock),
            0x3C => letter(b'u', shifted, self.caps_lock),
            0x2A => letter(b'v', shifted, self.caps_lock),
            0x1D => letter(b'w', shifted, self.caps_lock),
            0x22 => letter(b'x', shifted, self.caps_lock),
            0x35 => letter(b'y', shifted, self.caps_lock),
            0x1A => letter(b'z', shifted, self.caps_lock),
            0x16 => shifted_digit(b'1', b'!', shifted),
            0x1E => shifted_digit(b'2', b'@', shifted),
            0x26 => shifted_digit(b'3', b'#', shifted),
            0x25 => shifted_digit(b'4', b'$', shifted),
            0x2E => shifted_digit(b'5', b'%', shifted),
            0x36 => shifted_digit(b'6', b'^', shifted),
            0x3D => shifted_digit(b'7', b'&', shifted),
            0x3E => shifted_digit(b'8', b'*', shifted),
            0x46 => shifted_digit(b'9', b'(', shifted),
            0x45 => shifted_digit(b'0', b')', shifted),
            0x29 => b' ',
            0x5A => b'\n',
            0x66 => 0x08,
            0x0D => b'\t',
            0x4E => shifted_digit(b'-', b'_', shifted),
            0x55 => shifted_digit(b'=', b'+', shifted),
            0x54 => shifted_digit(b'[', b'{', shifted),
            0x5B => shifted_digit(b']', b'}', shifted),
            0x4C => shifted_digit(b';', b':', shifted),
            0x52 => shifted_digit(b'\'', b'"', shifted),
            0x41 => shifted_digit(b',', b'<', shifted),
            0x49 => shifted_digit(b'.', b'>', shifted),
            0x4A => shifted_digit(b'/', b'?', shifted),
            0x0E => shifted_digit(b'`', b'~', shifted),
            _ => 0,
        };

        if ascii == 0 {
            None
        } else {
            Some(KeyEvent { ascii, scan_code: byte })
        }
    }
}

const fn letter(lower: u8, shifted: bool, caps_lock: bool) -> u8 {
    if shifted ^ caps_lock { lower - 32 } else { lower }
}

const fn shifted_digit(normal: u8, shifted: u8, is_shifted: bool) -> u8 {
    if is_shifted { shifted } else { normal }
}

/// Hardware-facing driver shell. The 8042 transport, IRQ1 handler and queue
/// are intentionally not claimed as implemented by this decoder-only change.
pub struct Keyboard;

impl Keyboard {
    pub const fn new() -> Self { Self }
    pub fn init(&mut self) -> Result<&'static str, &str> { Ok("stub: no 8042 transport") }
    pub fn poll(&self) -> usize { 0 }
}

/// Return the global driver shell.
pub(crate) fn keyboard() -> &'static Keyboard {
    &KEYBOARD
}

static KEYBOARD: Keyboard = Keyboard::new();

#[cfg(test)]
mod tests {
    use super::{KeyEvent, ScanCodeSet2Decoder};

    #[test]
    fn decodes_make_code_and_ignores_break_code() {
        let mut decoder = ScanCodeSet2Decoder::new();
        assert_eq!(decoder.feed(0x1C), Some(KeyEvent { ascii: b'a', scan_code: 0x1C }));
        assert_eq!(decoder.feed(0xF0), None);
        assert_eq!(decoder.feed(0x1C), None);
    }

    #[test]
    fn shift_is_applied_until_its_break_code() {
        let mut decoder = ScanCodeSet2Decoder::new();
        assert_eq!(decoder.feed(0x12), None);
        assert_eq!(decoder.feed(0x1C).unwrap().ascii, b'A');
        assert_eq!(decoder.feed(0xF0), None);
        assert_eq!(decoder.feed(0x12), None);
        assert_eq!(decoder.feed(0x1C).unwrap().ascii, b'a');
    }

    #[test]
    fn caps_lock_and_shift_follow_xor_semantics() {
        let mut decoder = ScanCodeSet2Decoder::new();
        assert_eq!(decoder.feed(0x58), None);
        assert_eq!(decoder.feed(0x1C).unwrap().ascii, b'A');
        assert_eq!(decoder.feed(0x12), None);
        assert_eq!(decoder.feed(0x1C).unwrap().ascii, b'a');
    }

    #[test]
    fn decodes_shifted_digits_and_control_keys() {
        let mut decoder = ScanCodeSet2Decoder::new();
        decoder.feed(0x12);
        assert_eq!(decoder.feed(0x16).unwrap().ascii, b'!');
        assert_eq!(decoder.feed(0x5A).unwrap().ascii, b'\n');
        assert_eq!(decoder.feed(0x66).unwrap().ascii, 0x08);
    }

    #[test]
    fn ignores_extended_keys_and_unknown_scan_codes() {
        let mut decoder = ScanCodeSet2Decoder::new();
        assert_eq!(decoder.feed(0xE0), None);
        assert_eq!(decoder.feed(0x75), None);
        assert_eq!(decoder.feed(0x00), None);
        assert_eq!(decoder.feed(0x1C).unwrap().ascii, b'a');
    }
}
