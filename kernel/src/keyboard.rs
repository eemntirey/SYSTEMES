#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub ascii: u8,
    pub scan_code: u8,
}

pub struct Keyboard;

impl Keyboard {
    pub const fn new() -> Self { Self }
    pub fn init(&mut self) -> Result<&'static str, &str> { Ok("ok") }
    pub fn poll(&self) -> usize { 0 }
}

/// Return a mutable reference to the global keyboard driver.
pub(crate) fn keyboard() -> &'static Keyboard {
    // SAFETY: KEYBOARD is a static initialized at module load; caller guarantees
    // single-threaded (single-CPU) access.
    unsafe { &*(&KEYBOARD as *const Keyboard as *const Keyboard) }
}

static KEYBOARD: Keyboard = Keyboard::new();