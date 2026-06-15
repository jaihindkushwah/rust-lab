pub const PRICE_MULTIPLIER: i64 = 10_000; // 4 decimal places scaling factor (e.g., 150.2500 -> 1,502,500)

/// Converts a floating point number to a fixed-point integer.
#[inline(always)]
pub fn f64_to_fixed(val: f64) -> i64 {
    (val * PRICE_MULTIPLIER as f64) as i64
}

/// Converts a fixed-point integer back to a floating point number.
#[inline(always)]
pub fn fixed_to_f64(val: i64) -> f64 {
    val as f64 / PRICE_MULTIPLIER as f64
}

/// Fast parsing of a byte slice representation of a float directly into a fixed-point integer.
/// Avoids any heap allocation or standard library string parsing overhead.
#[inline(always)]
pub fn float_bytes_to_fixed(bytes: &[u8]) -> i64 {
    if bytes.is_empty() {
        return 0;
    }
    let mut integer_part: i64 = 0;
    let mut fractional_part: i64 = 0;
    let mut divisor: i64 = 1;
    let mut is_fraction = false;
    let mut is_negative = false;
    let mut idx = 0;

    if bytes[0] == b'-' {
        is_negative = true;
        idx += 1;
    } else if bytes[0] == b'+' {
        idx += 1;
    }

    while idx < bytes.len() {
        let b = bytes[idx];
        if b == b'.' {
            is_fraction = true;
            idx += 1;
            continue;
        }
        if b < b'0' || b > b'9' {
            // Stop parsing if non-numeric character is encountered
            break;
        }
        let digit = (b - b'0') as i64;
        if !is_fraction {
            integer_part = integer_part * 10 + digit;
        } else {
            if divisor < PRICE_MULTIPLIER {
                fractional_part = fractional_part * 10 + digit;
                divisor *= 10;
            }
        }
        idx += 1;
    }

    // Scale the fractional part to the target PRICE_MULTIPLIER
    // e.g., if multiplier is 10000 and we read ".25" (fractional=25, divisor=100),
    // then scaled fractional = 25 * (10000 / 100) = 2500.
    if divisor < PRICE_MULTIPLIER {
        fractional_part *= PRICE_MULTIPLIER / divisor;
    } else if divisor > PRICE_MULTIPLIER {
        fractional_part /= divisor / PRICE_MULTIPLIER;
    }

    let result = integer_part * PRICE_MULTIPLIER + fractional_part;
    if is_negative {
        -result
    } else {
        result
    }
}

/// Fast parsing of a byte slice representation of an integer to an unsigned 32-bit integer.
#[inline(always)]
pub fn bytes_to_u32(bytes: &[u8]) -> u32 {
    let mut result: u32 = 0;
    for &b in bytes {
        if b < b'0' || b > b'9' {
            break;
        }
        result = result * 10 + (b - b'0') as u32;
    }
    result
}

/// Reads the x86 processor Time Stamp Counter (TSC).
/// Falls back to system nanoseconds on non-x86_64 architectures.
#[inline(always)]
pub fn read_tsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::x86_64::_rdtsc()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let start = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        start.as_nanos() as u64
    }
}
