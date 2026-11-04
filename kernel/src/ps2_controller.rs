//! Bounded, fail-safe initialization of the legacy 8042 PS/2 controller.
//!
//! This module configures controller interfaces only. It deliberately leaves
//! controller IRQ bits disabled and does not issue keyboard/mouse device commands.

use core::arch::asm;

const DATA_PORT: u16 = 0x60;
const STATUS_COMMAND_PORT: u16 = 0x64;
const POLL_LIMIT: usize = 100_000;

const STATUS_OUTPUT_FULL: u8 = 1 << 0;
const STATUS_INPUT_FULL: u8 = 1 << 1;

const CMD_READ_CONFIG: u8 = 0x20;
const CMD_WRITE_CONFIG: u8 = 0x60;
const CMD_DISABLE_KEYBOARD: u8 = 0xAD;
const CMD_ENABLE_KEYBOARD: u8 = 0xAE;
const CMD_ENABLE_AUX: u8 = 0xA8;
const CMD_DISABLE_AUX: u8 = 0xA7;
const CMD_SELF_TEST: u8 = 0xAA;
const CMD_TEST_KEYBOARD: u8 = 0xAB;
const CMD_TEST_AUX: u8 = 0xA9;

const CONFIG_KEYBOARD_IRQ: u8 = 1 << 0;
const CONFIG_SYSTEM: u8 = 1 << 2;
const CONFIG_AUX_IRQ: u8 = 1 << 1;
const CONFIG_KEYBOARD_DISABLED: u8 = 1 << 4;
const CONFIG_AUX_DISABLED: u8 = 1 << 5;
const CONFIG_TRANSLATION: u8 = 1 << 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerError {
    InputTimeout,
    OutputTimeout,
    SelfTestFailed(u8),
    ConfigReadFailed,
    ConfigWriteFailed,
    KeyboardInterfaceFailed(u8),
    AuxiliaryInterfaceFailed(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerStatus {
    pub keyboard_interface_ready: bool,
    pub auxiliary_interface_detected: bool,
    pub irq_lines_disabled: bool,
    pub set1_translation_enabled: bool,
}

/// Initialize the controller with bounded waits. Does not enable IRQ1/IRQ12.
pub unsafe fn initialize() -> Result<ControllerStatus, ControllerError> {
    unsafe {
        // Stop both interfaces while changing controller configuration.
        command(CMD_DISABLE_KEYBOARD)?;
        command(CMD_DISABLE_AUX)?;
        flush_output();

        command(CMD_SELF_TEST)?;
        let self_test = read_data()?;
        if self_test != 0x55 {
            return Err(ControllerError::SelfTestFailed(self_test));
        }

        // The self-test may reset the configuration byte: read it afterwards.
        command(CMD_READ_CONFIG)?;
        let mut config = read_data().map_err(|_| ControllerError::ConfigReadFailed)?;
        config &= !(CONFIG_KEYBOARD_IRQ | CONFIG_AUX_IRQ);
        config |= CONFIG_KEYBOARD_DISABLED | CONFIG_AUX_DISABLED | CONFIG_TRANSLATION;
        write_config(config)?;

        command(CMD_TEST_KEYBOARD)?;
        let keyboard_test = read_data()?;
        if keyboard_test != 0x00 {
            return Err(ControllerError::KeyboardInterfaceFailed(keyboard_test));
        }

        // Some controllers do not implement an auxiliary port; report that
        // independently without making keyboard initialization fail.
        command(CMD_TEST_AUX)?;
        let auxiliary_detected = matches!(read_data(), Ok(0x00));

        // Enable only the keyboard clock. IRQ bits remain cleared.
        command(CMD_ENABLE_KEYBOARD)?;
        config &= !CONFIG_KEYBOARD_DISABLED;
        config |= CONFIG_AUX_DISABLED;
        write_config(config)?;

        Ok(ControllerStatus {
            keyboard_interface_ready: true,
            auxiliary_interface_detected: auxiliary_detected,
            irq_lines_disabled: config & (CONFIG_KEYBOARD_IRQ | CONFIG_AUX_IRQ) == 0,
            set1_translation_enabled: config & CONFIG_TRANSLATION != 0,
        })
    }
}


/// Enable both PS/2 interfaces and IRQ generation after IOAPIC routes are installed.
/// # Safety
/// Caller must have verified and unmasked IOAPIC routes for ISA IRQ1 and IRQ12.
pub unsafe fn enable_interrupts(auxiliary_ready: bool) -> Result<(), ControllerError> {
    unsafe {
        command(CMD_ENABLE_KEYBOARD)?;
        if auxiliary_ready {
            command(CMD_ENABLE_AUX)?;
        } else {
            command(CMD_DISABLE_AUX)?;
        }
        command(CMD_READ_CONFIG)?;
        let mut config = read_data().map_err(|_| ControllerError::ConfigReadFailed)?;
        config &= !CONFIG_KEYBOARD_DISABLED;
        config |= CONFIG_KEYBOARD_IRQ | CONFIG_TRANSLATION;
        if auxiliary_ready {
            config &= !CONFIG_AUX_DISABLED;
            config |= CONFIG_AUX_IRQ;
        } else {
            config |= CONFIG_AUX_DISABLED;
            config &= !CONFIG_AUX_IRQ;
        }
        write_config(config)?;
        command(CMD_READ_CONFIG)?;
        let verified = read_data().map_err(|_| ControllerError::ConfigReadFailed)?;
        let expected_irq_bits = CONFIG_KEYBOARD_IRQ
            | if auxiliary_ready { CONFIG_AUX_IRQ } else { 0 };
        let expected_disabled_bits = if auxiliary_ready { 0 } else { CONFIG_AUX_DISABLED };
        if verified & (CONFIG_KEYBOARD_IRQ | CONFIG_AUX_IRQ) != expected_irq_bits
            || verified & CONFIG_KEYBOARD_DISABLED != 0
            || verified & CONFIG_AUX_DISABLED != expected_disabled_bits
            || verified & CONFIG_TRANSLATION == 0 {
            return Err(ControllerError::ConfigWriteFailed);
        }
    }
    Ok(())
}

unsafe fn command(value: u8) -> Result<(), ControllerError> {
    unsafe {
        wait_input_empty()?;
        asm!("out dx, al", in("dx") STATUS_COMMAND_PORT, in("al") value, options(nomem, nostack, preserves_flags));
    }
    Ok(())
}

unsafe fn write_data(value: u8) -> Result<(), ControllerError> {
    unsafe {
        wait_input_empty()?;
        asm!("out dx, al", in("dx") DATA_PORT, in("al") value, options(nomem, nostack, preserves_flags));
    }
    Ok(())
}

unsafe fn read_data() -> Result<u8, ControllerError> {
    unsafe {
        for _ in 0..POLL_LIMIT {
            if read_status() & STATUS_OUTPUT_FULL != 0 {
                let value: u8;
                asm!("in al, dx", in("dx") DATA_PORT, out("al") value, options(nomem, nostack, preserves_flags));
                return Ok(value);
            }
            core::hint::spin_loop();
        }
    }
    Err(ControllerError::OutputTimeout)
}

unsafe fn wait_input_empty() -> Result<(), ControllerError> {
    unsafe {
        for _ in 0..POLL_LIMIT {
            if read_status() & STATUS_INPUT_FULL == 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }
    Err(ControllerError::InputTimeout)
}

unsafe fn read_status() -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", in("dx") STATUS_COMMAND_PORT, out("al") value, options(nomem, nostack, preserves_flags));
    }
    value
}

unsafe fn write_config(config: u8) -> Result<(), ControllerError> {
    unsafe {
        command(CMD_WRITE_CONFIG).map_err(|_| ControllerError::ConfigWriteFailed)?;
        write_data(config).map_err(|_| ControllerError::ConfigWriteFailed)
    }
}

unsafe fn flush_output() {
    unsafe {
        for _ in 0..POLL_LIMIT {
            if read_status() & STATUS_OUTPUT_FULL == 0 {
                break;
            }
            let _discarded: u8;
            asm!("in al, dx", in("dx") DATA_PORT, out("al") _discarded, options(nomem, nostack, preserves_flags));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_bits_keep_both_irq_lines_masked() {
        let config = CONFIG_SYSTEM | CONFIG_KEYBOARD_DISABLED | CONFIG_AUX_DISABLED | CONFIG_TRANSLATION;
        assert_eq!(config & (CONFIG_KEYBOARD_IRQ | CONFIG_AUX_IRQ), 0);
        assert_ne!(config & CONFIG_TRANSLATION, 0);
    }

    #[test]
    fn keyboard_enable_does_not_enable_auxiliary_or_irq_lines() {
        let mut config = CONFIG_KEYBOARD_DISABLED | CONFIG_AUX_DISABLED | CONFIG_TRANSLATION;
        config &= !CONFIG_KEYBOARD_DISABLED;
        config |= CONFIG_AUX_DISABLED;
        assert_eq!(config & CONFIG_KEYBOARD_DISABLED, 0);
        assert_ne!(config & CONFIG_AUX_DISABLED, 0);
        assert_eq!(config & (CONFIG_KEYBOARD_IRQ | CONFIG_AUX_IRQ), 0);
    }

    #[test]
    fn controller_timeouts_are_bounded() {
        assert!(POLL_LIMIT > 0);
        assert!(POLL_LIMIT <= 1_000_000);
    }
}
