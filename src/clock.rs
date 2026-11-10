//! Wall-clock date for the pre-image convention, without a date crate.
//!
//! The convention is `<target>.bak-YYYYMMDD` in **local** time, because the human reading a
//! journal entry reads local time. `libc` gives us the same calendar the shell engine used.

/// Today's local date as `YYYYMMDD`.
pub fn today() -> String {
    let mut now: libc::time_t = 0;
    unsafe {
        libc::time(&mut now);
    }
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        libc::localtime_r(&now, &mut tm);
    }
    format!("{:04}{:02}{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday)
}

#[cfg(test)]
mod tests {
    #[test]
    fn is_eight_digits() {
        let day = super::today();
        assert_eq!(day.len(), 8, "{day}");
        assert!(day.chars().all(|c| c.is_ascii_digit()), "{day}");
        let year: i32 = day[0..4].parse().unwrap();
        assert!(year >= 2024, "{day}");
    }
}
