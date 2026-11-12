//! Civil wall clock backed by the CMOS RTC (MC146818-compatible, ports 0x70/0x71).
//!
//! The kernel reads and validates the RTC once at boot, then advances the
//! captured civil time with the calibrated 100 Hz monotonic timer. `SYS_CLOCK`
//! returns a packed `u64` civil datetime. The RTC is assumed to hold UTC
//! (the QEMU/OVMF default `-rtc base=utc`); a firmware clock set to local time
//! shifts every reading by the same fixed offset, which is a documented
//! limitation until a timezone policy exists.

use core::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

/// Minimum supported year: the Unix epoch must stay representable and positive.
const MIN_YEAR: u32 = 1970;
/// Maximum supported year: one RTC century of service life.
const MAX_YEAR: u32 = 2170;

/// Number of full read attempts before the RTC is declared unstable.
const READ_ATTEMPTS: u32 = 32;
/// Bounded spin count while an RTC update cycle is in progress.
const UIP_WAIT_SPINS: u32 = 100_000;

/// Register offsets inside the CMOS RTC.
const RTC_SECONDS: u8 = 0x00;
const RTC_MINUTES: u8 = 0x02;
const RTC_HOURS: u8 = 0x04;
const RTC_DAY_OF_MONTH: u8 = 0x07;
const RTC_MONTH: u8 = 0x08;
const RTC_YEAR: u8 = 0x09;
const RTC_STATUS_A: u8 = 0x0A;
const RTC_STATUS_B: u8 = 0x0B;
const RTC_CENTURY: u8 = 0x32;

/// Status register A bit 7: update-in-progress flag.
const STATUS_A_UPDATE_IN_PROGRESS: u8 = 0x80;
/// Status register B bit 2: data mode (0 = BCD, 1 = binary).
const STATUS_B_DATA_MODE_BINARY: u8 = 0x04;
/// Status register B bit 1: hour mode (0 = 12-hour, 1 = 24-hour).
const STATUS_B_HOUR_MODE_24: u8 = 0x02;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RtcError {
    /// Every sample attempt raced an RTC update cycle or produced garbage.
    UnstableRead,
    /// A clean sample was read but its values are outside the supported ranges.
    InvalidDateTime,
}

/// One validated civil datetime (UTC as read from the RTC).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DateTime {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

// SYS_CLOCK packed layout (little-endian bytes of the returned u64):
//   byte 0   = second (0..=59)
//   byte 1   = minute (0..=59)
//   byte 2   = hour   (0..=23)
//   byte 3   = day    (1..=31)
//   byte 4   = month  (1..=12)
//   bytes 5-7 = year  (u24, little-endian; 1970..=2170)
const PACK_YEAR_SHIFT: u64 = 40;

/// Pack a validated [`DateTime`] into the `SYS_CLOCK` return value.
#[must_use]
pub const fn pack_datetime(datetime: DateTime) -> u64 {
    (datetime.second as u64)
        | (datetime.minute as u64) << 8
        | (datetime.hour as u64) << 16
        | (datetime.day as u64) << 24
        | (datetime.month as u64) << 32
        | (datetime.year as u64) << PACK_YEAR_SHIFT
}

/// Unpack a `SYS_CLOCK` value back into its components (host-test helper).
#[must_use]
pub const fn unpack_datetime(packed: u64) -> DateTime {
    DateTime {
        second: (packed & 0xFF) as u32,
        minute: ((packed >> 8) & 0xFF) as u32,
        hour: ((packed >> 16) & 0xFF) as u32,
        day: ((packed >> 24) & 0xFF) as u32,
        month: ((packed >> 32) & 0xFF) as u32,
        year: ((packed >> PACK_YEAR_SHIFT) & 0xFF_FFFF) as u32,
    }
}

/// Convert a raw BCD register value to binary (`0x26` -> `26`).
#[must_use]
pub const fn bcd_to_binary(value: u8) -> u8 {
    ((value >> 4) & 0x0F).wrapping_mul(10).wrapping_add(value & 0x0F)
}

/// Normalize a raw RTC hour register to 0..=23, honoring BCD/binary and 12/24-hour modes.
#[must_use]
pub fn normalize_hour(raw: u8, binary_mode: bool, hour24_mode: bool) -> u32 {
    let afternoon = raw & 0x80 != 0;
    let code = raw & 0x7F;
    let value = if binary_mode { code as u32 } else { bcd_to_binary(code) as u32 };
    if hour24_mode {
        return value;
    }
    if value == 12 {
        if afternoon { 12 } else { 0 }
    } else if afternoon {
        value + 12
    } else {
        value
    }
}

/// Normalize a non-hour RTC register (seconds, minutes, day, month, year) to binary.
#[must_use]
pub fn normalize_field(raw: u8, binary_mode: bool) -> u32 {
    if binary_mode { raw as u32 } else { bcd_to_binary(raw) as u32 }
}

const fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Number of days in a month for the given year.
#[must_use]
pub const fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        },
        _ => 0,
    }
}

/// Validate raw RTC fields against the supported ranges.
#[must_use]
pub fn validate_datetime(
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Result<DateTime, RtcError> {
    if !(MIN_YEAR..=MAX_YEAR).contains(&year)
        || !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return Err(RtcError::InvalidDateTime);
    }
    Ok(DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

/// Days since 1970-01-01 for a proleptic Gregorian civil date (Howard Hinnant's algorithm).
#[must_use]
pub const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_shift = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_shift + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Proleptic Gregorian civil date for a count of days since 1970-01-01.
#[must_use]
pub const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 { shifted } else { shifted - 146_096 } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    (year + if month <= 2 { 1 } else { 0 }, month, day)
}

/// Convert a validated datetime to seconds since the Unix epoch (UTC).
#[must_use]
pub fn to_epoch_seconds(datetime: DateTime) -> i64 {
    days_from_civil(datetime.year as i64, datetime.month as i64, datetime.day as i64) * 86_400
        + datetime.hour as i64 * 3600
        + datetime.minute as i64 * 60
        + datetime.second as i64
}

/// Convert seconds since the Unix epoch (UTC) back to a civil datetime.
#[must_use]
pub fn from_epoch_seconds(epoch: i64) -> DateTime {
    let days = epoch.div_euclid(86_400);
    let remainder = epoch.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    DateTime {
        year: year as u32,
        month: month as u32,
        day: day as u32,
        hour: (remainder / 3600) as u32,
        minute: ((remainder % 3600) / 60) as u32,
        second: (remainder % 60) as u32,
    }
}

/// Format as `YYYY-MM-DD HH:MM:SS` into `out` and return the 19 written bytes.
///
/// Returns 0 when `out` is too small. Digits are zero-padded.
#[must_use]
pub fn format_datetime(datetime: &DateTime, out: &mut [u8]) -> usize {
    if out.len() < 19 {
        return 0;
    }
    let mut position = 0;
    position += write_decimal(&mut out[position..], datetime.year, 4);
    out[position] = b'-';
    position += 1;
    position += write_decimal(&mut out[position..], datetime.month, 2);
    out[position] = b'-';
    position += 1;
    position += write_decimal(&mut out[position..], datetime.day, 2);
    out[position] = b' ';
    position += 1;
    position += write_decimal(&mut out[position..], datetime.hour, 2);
    out[position] = b':';
    position += 1;
    position += write_decimal(&mut out[position..], datetime.minute, 2);
    out[position] = b':';
    position += 1;
    position += write_decimal(&mut out[position..], datetime.second, 2);
    position
}

/// Write `value` zero-padded to `width` decimal digits; returns the bytes written.
fn write_decimal(out: &mut [u8], value: u32, width: usize) -> usize {
    if width == 0 || width > out.len() {
        return 0;
    }
    let mut digits = [0u8; 10];
    let mut remaining = value;
    let mut count = 0;
    loop {
        digits[count] = b'0' + (remaining % 10) as u8;
        count += 1;
        remaining /= 10;
        if remaining == 0 || count == digits.len() {
            break;
        }
    }
    if count > width {
        // Truncate the most significant digits: the caller validated the ranges.
        return 0;
    }
    for index in 0..width {
        let digit_position = width - 1 - index;
        let digit_source = if digit_position >= count {
            b'0'
        } else {
            digits[digit_position]
        };
        out[index] = digit_source;
    }
    width
}

// ---------------------------------------------------------------------------
// Boot state
// ---------------------------------------------------------------------------

static BOOT_EPOCH_SECONDS: AtomicI64 = AtomicI64::new(0);
static BOOT_MONOTONIC_TICKS: AtomicU64 = AtomicU64::new(0);
static RTC_READY: AtomicBool = AtomicBool::new(false);

/// Read and validate the RTC once at boot and anchor the civil clock to the
/// monotonic timer. Must be called from ring 0 on the bootstrap CPU.
pub fn initialize() -> Result<DateTime, RtcError> {
    let datetime = read_cmos_datetime()?;
    let epoch = to_epoch_seconds(datetime);
    // Publish the anchors in dependency order; RTC_READY gates readers last so
    // no reader can observe a partial anchor pair.
    BOOT_EPOCH_SECONDS.store(epoch, Ordering::Release);
    BOOT_MONOTONIC_TICKS.store(crate::clock::ticks(), Ordering::Release);
    RTC_READY.store(true, Ordering::Release);
    Ok(datetime)
}

/// Seconds since the Unix epoch, advancing the boot RTC sample with the
/// calibrated monotonic timer. `None` until `initialize` succeeds.
#[must_use]
pub fn now_epoch_seconds() -> Option<i64> {
    if !RTC_READY.load(Ordering::Acquire) {
        return None;
    }
    let boot_epoch = BOOT_EPOCH_SECONDS.load(Ordering::Acquire);
    let boot_ticks = BOOT_MONOTONIC_TICKS.load(Ordering::Acquire);
    let elapsed = crate::clock::ticks_to_seconds(crate::clock::ticks().saturating_sub(boot_ticks));
    Some(boot_epoch + elapsed as i64)
}

/// Packed `SYS_CLOCK` value, or `None` while the civil clock is unavailable.
#[must_use]
pub fn now_packed() -> Option<u64> {
    Some(pack_datetime(from_epoch_seconds(now_epoch_seconds()?)))
}

// ---------------------------------------------------------------------------
// Hardware access
// ---------------------------------------------------------------------------

/// Read one CMOS RTC register. Bit 7 of the index port is kept clear so NMI
/// delivery stays enabled during the access.
fn read_cmos_register(index: u8) -> u8 {
    let value: u8;
    // SAFETY: ports 0x70 (index) and 0x71 (data) are the documented CMOS RTC
    // access path; the write and the read are a single paired port access.
    unsafe {
        core::arch::asm!(
            "out 0x70, al",
            "in al, 0x71",
            in("al") index & 0x7F,
            lateout("al") value,
            options(nomem, nostack)
        );
    }
    value
}

fn update_in_progress() -> bool {
    read_cmos_register(RTC_STATUS_A) & STATUS_A_UPDATE_IN_PROGRESS != 0
}

/// Read the RTC with the standard update-cycle protocol: wait out any in-progress
/// update, sample every register, then re-check the update flag; retry on a race.
fn read_cmos_datetime() -> Result<DateTime, RtcError> {
    let status_b = read_cmos_register(RTC_STATUS_B);
    let binary_mode = status_b & STATUS_B_DATA_MODE_BINARY != 0;
    let hour24_mode = status_b & STATUS_B_HOUR_MODE_24 != 0;

    let mut last_error = RtcError::UnstableRead;
    for _ in 0..READ_ATTEMPTS {
        // Bounded wait while the internal update cycle blocks consistent reads.
        let mut spins = 0;
        while update_in_progress() {
            spins += 1;
            if spins >= UIP_WAIT_SPINS {
                break;
            }
        }

        let second_raw = read_cmos_register(RTC_SECONDS);
        let minute_raw = read_cmos_register(RTC_MINUTES);
        let hour_raw = read_cmos_register(RTC_HOURS);
        let day_raw = read_cmos_register(RTC_DAY_OF_MONTH);
        let month_raw = read_cmos_register(RTC_MONTH);
        let year_raw = read_cmos_register(RTC_YEAR);
        let century_raw = read_cmos_register(RTC_CENTURY);

        if update_in_progress() {
            // The update cycle started mid-sample; discard and retry.
            last_error = RtcError::UnstableRead;
            continue;
        }

        let year_register = normalize_field(year_raw, binary_mode);
        let century = if century_raw == 0 {
            // Some boards do not implement the ACPI century register.
            if year_register >= 70 { 19 } else { 20 }
        } else {
            normalize_field(century_raw, binary_mode)
        };
        last_error = match validate_datetime(
            century * 100 + year_register,
            normalize_field(month_raw, binary_mode),
            normalize_field(day_raw, binary_mode),
            normalize_hour(hour_raw, binary_mode, hour24_mode),
            normalize_field(minute_raw, binary_mode),
            normalize_field(second_raw, binary_mode),
        ) {
            Ok(datetime) => return Ok(datetime),
            Err(error) => error,
        };
    }
    Err(last_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bcd_values_convert_to_binary() {
        assert_eq!(bcd_to_binary(0x00), 0);
        assert_eq!(bcd_to_binary(0x09), 9);
        assert_eq!(bcd_to_binary(0x26), 26);
        assert_eq!(bcd_to_binary(0x59), 59);
        assert_eq!(bcd_to_binary(0x20), 20);
        assert_eq!(bcd_to_binary(0x99), 99);
    }

    #[test]
    fn hours_normalize_across_binary_bcd_and_12_24_modes() {
        // 24-hour binary: raw values pass through.
        assert_eq!(normalize_hour(0, true, true), 0);
        assert_eq!(normalize_hour(23, true, true), 23);
        // 24-hour BCD.
        assert_eq!(normalize_hour(0x23, false, true), 23);
        // 12-hour binary: 12 PM is noon, 12 AM is midnight, 1 PM is 13:00.
        assert_eq!(normalize_hour(12 | 0x80, true, false), 12);
        assert_eq!(normalize_hour(12, true, false), 0);
        assert_eq!(normalize_hour(1 | 0x80, true, false), 13);
        assert_eq!(normalize_hour(11, true, false), 11);
        // 12-hour BCD: 11 PM is 23:00 and 1 PM is 13:00.
        assert_eq!(normalize_hour(0x11 | 0x80, false, false), 23);
        assert_eq!(normalize_hour(0x01 | 0x80, false, false), 13);
        assert_eq!(normalize_hour(0x12, false, false), 0);
        assert_eq!(normalize_hour(0x12 | 0x80, false, false), 12);
    }

    #[test]
    fn fields_convert_in_bcd_and_binary_modes() {
        assert_eq!(normalize_field(0x12, false), 12);
        assert_eq!(normalize_field(12, true), 12);
        assert_eq!(normalize_field(0x26, false), 26);
    }

    #[test]
    fn month_lengths_follow_the_leap_rules() {
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 13), 0);
    }

    #[test]
    fn validation_rejects_out_of_range_fields() {
        assert!(validate_datetime(2026, 11, 12, 10, 30, 0).is_ok());
        assert_eq!(validate_datetime(1969, 12, 31, 23, 59, 59), Err(RtcError::InvalidDateTime));
        assert_eq!(validate_datetime(2171, 1, 1, 0, 0, 0), Err(RtcError::InvalidDateTime));
        assert_eq!(validate_datetime(2026, 0, 1, 0, 0, 0), Err(RtcError::InvalidDateTime));
        assert_eq!(validate_datetime(2026, 13, 1, 0, 0, 0), Err(RtcError::InvalidDateTime));
        assert_eq!(validate_datetime(2026, 2, 30, 0, 0, 0), Err(RtcError::InvalidDateTime));
        assert_eq!(validate_datetime(2026, 1, 32, 0, 0, 0), Err(RtcError::InvalidDateTime));
        assert_eq!(validate_datetime(2026, 1, 1, 24, 0, 0), Err(RtcError::InvalidDateTime));
        assert_eq!(validate_datetime(2026, 1, 1, 0, 60, 0), Err(RtcError::InvalidDateTime));
        assert_eq!(validate_datetime(2026, 1, 1, 0, 0, 60), Err(RtcError::InvalidDateTime));
    }

    #[test]
    fn epoch_conversions_match_known_reference_points() {
        // 1970-01-01T00:00:00Z is epoch 0.
        let epoch_zero = validate_datetime(1970, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(to_epoch_seconds(epoch_zero), 0);
        assert_eq!(from_epoch_seconds(0), epoch_zero);

        // 2000-02-29T12:00:00Z (leap day).
        let leap_day = validate_datetime(2000, 2, 29, 12, 0, 0).unwrap();
        assert_eq!(to_epoch_seconds(leap_day), 951_825_600);

        // 2026-11-12T10:30:00Z.
        let reference = validate_datetime(2026, 11, 12, 10, 30, 0).unwrap();
        assert_eq!(to_epoch_seconds(reference), 1_794_479_400);

        // 2038-01-19T03:14:07Z is the signed 32-bit limit and must stay valid.
        let beyond = validate_datetime(2038, 1, 19, 3, 14, 7).unwrap();
        assert_eq!(to_epoch_seconds(beyond), 2_147_483_647);
    }

    #[test]
    fn epoch_round_trip_preserves_every_field() {
        for epoch in [0i64, 1, 59, 60, 3600, 86_399, 86_400, 951_825_600, 1_794_479_400, 4_000_000_000] {
            let datetime = from_epoch_seconds(epoch);
            assert_eq!(to_epoch_seconds(datetime), epoch, "epoch {epoch} did not round trip");
        }
    }

    #[test]
    fn days_from_civil_round_trips_across_leap_boundaries() {
        for days in [-1i64, 0, 365, 365 * 2, 10_957, 18_497, 20_000, 30_000] {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days, "day {days} did not round trip");
        }
        // 2000-01-01 is exactly 10957 days after the epoch.
        assert_eq!(days_from_civil(2000, 1, 1), 10_957);
        // 2024-02-29 exists and maps to a single day index.
        let leap_2024 = days_from_civil(2024, 2, 29);
        assert_eq!(days_from_civil(2024, 3, 1), leap_2024 + 1);
    }

    #[test]
    fn packed_layout_keeps_every_field_byte_aligned() {
        let datetime = validate_datetime(2026, 11, 12, 10, 30, 5).unwrap();
        let packed = pack_datetime(datetime);
        assert_eq!(packed & 0xFF, 5);
        assert_eq!((packed >> 8) & 0xFF, 30);
        assert_eq!((packed >> 16) & 0xFF, 10);
        assert_eq!((packed >> 24) & 0xFF, 12);
        assert_eq!((packed >> 32) & 0xFF, 11);
        assert_eq!((packed >> PACK_YEAR_SHIFT) & 0xFF_FFFF, 2026);
        assert_eq!(unpack_datetime(packed), datetime);
    }

    #[test]
    fn formatting_zero_pads_every_field() {
        let datetime = validate_datetime(2026, 11, 12, 10, 30, 5).unwrap();
        let mut buffer = [0u8; 19];
        assert_eq!(format_datetime(&datetime, &mut buffer), 19);
        assert_eq!(&buffer, b"2026-11-12 10:30:05");

        let midnight = validate_datetime(1970, 1, 1, 0, 0, 0).unwrap();
        let mut small = [0u8; 19];
        assert_eq!(format_datetime(&midnight, &mut small), 19);
        assert_eq!(&small, b"1970-01-01 00:00:00");

        let mut tiny = [0u8; 18];
        assert_eq!(format_datetime(&datetime, &mut tiny), 0);
    }
}
