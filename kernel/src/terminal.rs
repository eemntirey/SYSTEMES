//! GOP text terminal et une petite console shell polled par série.

use crate::{
    BootInfo,
    console::debug_write,
    serial::{self, SerialError},
};

const CELL_WIDTH: usize = 6;
const CELL_HEIGHT: usize = 9;
const FONT_WIDTH: usize = 5;
const FONT_HEIGHT: usize = 7;
const INPUT_CAPACITY: usize = 128;
const FOREGROUND: (u8, u8, u8) = (230, 238, 248);
const BACKGROUND: (u8, u8, u8) = (12, 18, 28);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalError {
    InvalidFramebuffer,
    UnsupportedPixelFormat,
    FramebufferSizeOverflow,
    Serial(SerialError),
}

struct Framebuffer {
    pixels: *mut u32,
    width: usize,
    height: usize,
    stride: usize,
    bgr: bool,
    cursor_x: usize,
    cursor_y: usize,
}

impl Framebuffer {
    unsafe fn new(info: &BootInfo) -> Result<Self, TerminalError> {
        let width = info.framebuffer_width as usize;
        let height = info.framebuffer_height as usize;
        let stride = info.framebuffer_stride as usize;
        if info.framebuffer_address == 0
            || width < CELL_WIDTH
            || height < CELL_HEIGHT
            || stride < width
            || info.framebuffer_address % core::mem::align_of::<u32>() as u64 != 0
        {
            return Err(TerminalError::InvalidFramebuffer);
        }
        let bgr = match info.framebuffer_pixel_format {
            0 => false,
            1 => true,
            _ => return Err(TerminalError::UnsupportedPixelFormat),
        };
        let last_pixel = (height - 1)
            .checked_mul(stride)
            .and_then(|row| row.checked_add(width))
            .ok_or(TerminalError::FramebufferSizeOverflow)?;
        let byte_length = last_pixel
            .checked_mul(core::mem::size_of::<u32>())
            .ok_or(TerminalError::FramebufferSizeOverflow)?;
        let framebuffer_end = info
            .framebuffer_address
            .checked_add(byte_length as u64)
            .ok_or(TerminalError::FramebufferSizeOverflow)?;
        if framebuffer_end > crate::paging::IDENTITY_MAP_BYTES {
            return Err(TerminalError::InvalidFramebuffer);
        }

        Ok(Self {
            pixels: info.framebuffer_address as *mut u32,
            width,
            height,
            stride,
            bgr,
            cursor_x: 0,
            cursor_y: 0,
        })
    }

    fn color(&self, (red, green, blue): (u8, u8, u8)) -> u32 {
        if self.bgr {
            u32::from(blue) | (u32::from(green) << 8) | (u32::from(red) << 16)
        } else {
            u32::from(red) | (u32::from(green) << 8) | (u32::from(blue) << 16)
        }
    }

    fn clear(&mut self) {
        let background = self.color(BACKGROUND);
        for y in 0..self.height {
            for x in 0..self.width {
                // SAFETY: The validated GOP buffer covers all visible pixels.
                unsafe {
                    core::ptr::write_volatile(self.pixels.add(y * self.stride + x), background)
                };
            }
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    fn put_byte(&mut self, byte: u8) {
        match byte {
            b'\r' => {}
            b'\n' => self.newline(),
            8 | 127 => self.backspace(),
            _ => self.draw_char(byte),
        }
    }

    fn draw_char(&mut self, byte: u8) {
        if self.cursor_x + CELL_WIDTH > self.width {
            self.newline();
        }
        if self.cursor_y + CELL_HEIGHT > self.height {
            self.scroll();
        }

        let rows = glyph(byte.to_ascii_uppercase());
        let foreground = self.color(FOREGROUND);
        let background = self.color(BACKGROUND);
        for y in 0..CELL_HEIGHT {
            for x in 0..CELL_WIDTH {
                let set = y < FONT_HEIGHT
                    && x < FONT_WIDTH
                    && (rows[y] & (1 << (FONT_WIDTH - 1 - x))) != 0;
                let pixel = if set { foreground } else { background };
                let offset = (self.cursor_y + y) * self.stride + self.cursor_x + x;
                // SAFETY: Cursor wrapping and geometry validation keep the cell within the GOP buffer.
                unsafe { core::ptr::write_volatile(self.pixels.add(offset), pixel) };
            }
        }
        self.cursor_x += CELL_WIDTH;
    }

    fn newline(&mut self) {
        self.cursor_x = 0;
        self.cursor_y += CELL_HEIGHT;
        if self.cursor_y + CELL_HEIGHT > self.height {
            self.scroll();
        }
    }

    fn backspace(&mut self) {
        if self.cursor_x >= CELL_WIDTH {
            self.cursor_x -= CELL_WIDTH;
        } else if self.cursor_y >= CELL_HEIGHT {
            self.cursor_y -= CELL_HEIGHT;
            self.cursor_x = (self.width / CELL_WIDTH - 1) * CELL_WIDTH;
        } else {
            return;
        }
        self.draw_char(b' ');
        self.cursor_x -= CELL_WIDTH;
    }

    fn scroll(&mut self) {
        let rows_to_keep = self.height - CELL_HEIGHT;
        for y in 0..rows_to_keep {
            for x in 0..self.width {
                let source = (y + CELL_HEIGHT) * self.stride + x;
                let destination = y * self.stride + x;
                // SAFETY: Both offsets refer to validated visible framebuffer pixels.
                let pixel = unsafe { core::ptr::read_volatile(self.pixels.add(source)) };
                // SAFETY: Destination is within the visible framebuffer.
                unsafe { core::ptr::write_volatile(self.pixels.add(destination), pixel) };
            }
        }
        let background = self.color(BACKGROUND);
        for y in rows_to_keep..self.height {
            for x in 0..self.width {
                // SAFETY: This row is within the validated visible framebuffer.
                unsafe {
                    core::ptr::write_volatile(self.pixels.add(y * self.stride + x), background)
                };
            }
        }
        self.cursor_y = rows_to_keep - rows_to_keep % CELL_HEIGHT;
    }
}

pub struct Console {
    framebuffer: Option<Framebuffer>,
}

impl Console {
    /// Initialize COM1 and attach the GOP buffer when one was provided.
    ///
    /// # Safety
    ///
    /// A nonzero framebuffer address in `info` must identify a writable,
    /// identity-mapped GOP framebuffer of the declared geometry.
    pub unsafe fn new(info: &BootInfo) -> Result<Self, TerminalError> {
        // SAFETY: COM1 is the conventional serial UART used for shell input/output.
        unsafe { serial::initialize() };
        // SAFETY: The caller guarantees the boot handoff and framebuffer mapping are valid.
        let framebuffer = if info.framebuffer_address == 0 {
            None
        } else {
            match unsafe { Framebuffer::new(info) } {
                Ok(framebuffer) => Some(framebuffer),
                Err(error) => {
                    report_framebuffer_issue(error);
                    None
                }
            }
        };
        Ok(Self { framebuffer })
    }

    pub fn clear(&mut self) -> Result<(), TerminalError> {
        if let Some(framebuffer) = &mut self.framebuffer {
            framebuffer.clear();
        }
        self.write_serial(b"\x1b[2J\x1b[H")
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        for &byte in bytes {
            if let Some(framebuffer) = &mut self.framebuffer {
                framebuffer.put_byte(byte);
            }
            serial::write_byte(byte).map_err(TerminalError::Serial)?;
        }
        Ok(())
    }

    fn write_serial(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        for &byte in bytes {
            serial::write_byte(byte).map_err(TerminalError::Serial)?;
        }
        Ok(())
    }

    fn erase_input_char(&mut self) -> Result<(), TerminalError> {
        if let Some(framebuffer) = &mut self.framebuffer {
            framebuffer.backspace();
        }
        self.write_serial(b"\x08 \x08")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command<'a> {
    Empty,
    Help,
    Clear,
    Uname,
    Echo(&'a str),
    Unknown,
}

fn parse_command(line: &str) -> Command<'_> {
    let line = line.trim();
    if line.is_empty() {
        return Command::Empty;
    }
    let (name, arguments) = line
        .split_once(char::is_whitespace)
        .map_or((line, ""), |(name, rest)| (name, rest.trim()));
    if name.eq_ignore_ascii_case("help") {
        Command::Help
    } else if name.eq_ignore_ascii_case("clear") {
        Command::Clear
    } else if name.eq_ignore_ascii_case("uname") {
        Command::Uname
    } else if name.eq_ignore_ascii_case("echo") {
        Command::Echo(arguments)
    } else {
        Command::Unknown
    }
}

struct Shell {
    input: [u8; INPUT_CAPACITY],
    length: usize,
    overflowed: bool,
    last_was_cr: bool,
}

impl Shell {
    const fn new() -> Self {
        Self {
            input: [0; INPUT_CAPACITY],
            length: 0,
            overflowed: false,
            last_was_cr: false,
        }
    }

    fn execute(&mut self, console: &mut Console) -> Result<(), TerminalError> {
        if self.overflowed {
            console.write_bytes(b"INPUT TOO LONG; LINE DISCARDED\r\n")?;
        } else {
            let line = core::str::from_utf8(&self.input[..self.length])
                .expect("shell input accepts ASCII bytes only");
            match parse_command(line) {
                Command::Empty => {}
                Command::Help => console.write_bytes(b"COMMANDS: HELP CLEAR UNAME ECHO\r\n")?,
                Command::Clear => console.clear()?,
                Command::Uname => console.write_bytes(b"SYSTEMES KERNEL T12\r\n")?,
                Command::Echo(text) => {
                    console.write_bytes(text.as_bytes())?;
                    console.write_bytes(b"\r\n")?;
                }
                Command::Unknown => console.write_bytes(b"UNKNOWN COMMAND; TYPE HELP\r\n")?,
            }
        }
        self.length = 0;
        self.overflowed = false;
        console.write_bytes(b"> ")
    }

    fn accept_byte(&mut self, byte: u8, console: &mut Console) -> Result<(), TerminalError> {
        if byte == b'\n' && self.last_was_cr {
            self.last_was_cr = false;
            return Ok(());
        }
        self.last_was_cr = byte == b'\r';
        match byte {
            b'\r' | b'\n' => {
                console.write_bytes(b"\r\n")?;
                self.execute(console)
            }
            8 | 127 if self.length > 0 && !self.overflowed => {
                self.length -= 1;
                console.erase_input_char()
            }
            8 | 127 => Ok(()),
            0x20..=0x7e if self.length < self.input.len() && !self.overflowed => {
                self.input[self.length] = byte;
                self.length += 1;
                console.write_bytes(&[byte])
            }
            0x20..=0x7e => {
                self.overflowed = true;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Run the shell until the machine is reset or powered off.
pub fn run(info: &BootInfo, timer_available: bool) -> ! {
    // SAFETY: `_start` validated the UEFI handoff and installed identity mappings.
    let mut console = match unsafe { Console::new(info) } {
        Ok(console) => console,
        Err(error) => terminal_failure(error),
    };
    if let Err(error) = console.clear() {
        terminal_failure(error);
    }
    if let Err(error) = console.write_bytes(b"SYSTEMES T12 CONSOLE\r\nTYPE HELP FOR COMMANDS\r\n> ") {
        terminal_failure(error);
    }

    let mut shell = Shell::new();
    loop {
        if let Some(byte) = serial::try_read_byte() {
            if let Err(error) = shell.accept_byte(byte, &mut console) {
                terminal_failure(error);
            }
        } else if timer_available {
            // SAFETY: A configured Local APIC timer periodically wakes this serial poll loop.
            unsafe {
                core::arch::asm!("hlt", options(nomem, nostack));
            }
        } else {
            core::hint::spin_loop();
        }
    }
}

fn terminal_failure(error: TerminalError) -> ! {
    match error {
        TerminalError::Serial(SerialError::TransmitTimeout) => unsafe {
            debug_write(b"SYSTEMES_KERNEL_FAIL: COM1 transmit timeout\r\n");
        },
        TerminalError::InvalidFramebuffer => unsafe {
            debug_write(b"SYSTEMES_KERNEL_FAIL: invalid GOP framebuffer geometry\r\n");
        },
        TerminalError::UnsupportedPixelFormat => unsafe {
            debug_write(b"SYSTEMES_KERNEL_FAIL: unsupported GOP pixel format\r\n");
        },
        TerminalError::FramebufferSizeOverflow => unsafe {
            debug_write(b"SYSTEMES_KERNEL_FAIL: GOP framebuffer size overflow\r\n");
        },
    }
    loop {
        // SAFETY: A terminal I/O error is fatal to the interactive shell.
        unsafe {
            core::arch::asm!("cli; hlt", options(nomem, nostack));
        }
    }
}

fn report_framebuffer_issue(error: TerminalError) {
    match error {
        TerminalError::InvalidFramebuffer => unsafe {
            debug_write(b"SYSTEMES: invalid GOP framebuffer; using serial terminal only\r\n");
        },
        TerminalError::UnsupportedPixelFormat => unsafe {
            debug_write(b"SYSTEMES: unsupported GOP pixel format; using serial terminal only\r\n");
        },
        TerminalError::FramebufferSizeOverflow => unsafe {
            debug_write(b"SYSTEMES: GOP framebuffer size overflow; using serial terminal only\r\n");
        },
        TerminalError::Serial(SerialError::TransmitTimeout) => {
            terminal_failure_serial_timeout();
        }
    }
}

fn terminal_failure_serial_timeout() -> ! {
    unsafe { debug_write(b"SYSTEMES_KERNEL_FAIL: COM1 transmit timeout\r\n") };
    loop {
        // SAFETY: COM1 failed; stop instead of silently losing shell output.
        unsafe {
            core::arch::asm!("cli; hlt", options(nomem, nostack));
        }
    }
}

fn glyph(byte: u8) -> [u8; FONT_HEIGHT] {
    const LETTERS: [[u8; FONT_HEIGHT]; 26] = [
        [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        [0x07, 0x02, 0x02, 0x02, 0x12, 0x12, 0x0c],
        [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x01, 0x0e],
    ];
    const DIGITS: [[u8; FONT_HEIGHT]; 10] = [
        [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        [0x1e, 0x01, 0x01, 0x0e, 0x01, 0x01, 0x1e],
        [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        [0x1f, 0x10, 0x10, 0x1e, 0x01, 0x01, 0x1e],
        [0x0e, 0x10, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x01, 0x0e],
    ];

    match byte {
        b'A'..=b'Z' => LETTERS[(byte - b'A') as usize],
        b'0'..=b'9' => DIGITS[(byte - b'0') as usize],
        b'>' => [0x10, 0x08, 0x04, 0x02, 0x04, 0x08, 0x10],
        b'<' => [0x01, 0x02, 0x04, 0x08, 0x04, 0x02, 0x01],
        b':' => [0x00, 0x04, 0x04, 0x00, 0x04, 0x04, 0x00],
        b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        b',' => [0x00, 0x00, 0x00, 0x00, 0x04, 0x04, 0x08],
        b'-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        b'_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
        b'!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        b'?' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        b'=' => [0x00, 0x1f, 0x00, 0x1f, 0x00, 0x00, 0x00],
        b'(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        b')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        b' ' => [0; FONT_HEIGHT],
        _ => [0x1f, 0x11, 0x02, 0x04, 0x04, 0x00, 0x04],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shell_commands_case_insensitively() {
        assert_eq!(parse_command("help"), Command::Help);
        assert_eq!(parse_command("  UnAmE  "), Command::Uname);
        assert_eq!(
            parse_command("echo hello kernel"),
            Command::Echo("hello kernel")
        );
        assert_eq!(parse_command(""), Command::Empty);
    }

    #[test]
    fn unknown_shell_commands_are_explicit() {
        assert_eq!(parse_command("format disk"), Command::Unknown);
    }

    #[test]
    fn font_covers_letters_digits_and_terminal_punctuation() {
        assert_ne!(glyph(b'A'), [0; FONT_HEIGHT]);
        assert_ne!(glyph(b'9'), [0; FONT_HEIGHT]);
        assert_ne!(glyph(b'>'), [0; FONT_HEIGHT]);
        assert_eq!(glyph(b' '), [0; FONT_HEIGHT]);
    }
}