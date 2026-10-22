//! The order the macOS locale (System Settings → General → Language & Region) reads dates in. Month and
//! weekday names stay English; only the order of the day and the month follows the region.

/// Whether a date reads `Oct 21, 2026` or `21 Oct 2026`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateOrder {
    MonthFirst,
    DayFirst,
}

impl DateOrder {
    /// The current locale's order, read once per run; month first where it cannot be read. Tests always see
    /// month first, so their labels do not depend on the machine's region.
    pub fn system() -> DateOrder {
        static ORDER: std::sync::OnceLock<DateOrder> = std::sync::OnceLock::new();
        *ORDER.get_or_init(|| read().map_or(DateOrder::MonthFirst, |p| order_of(&p)))
    }
}

/// The current locale's pattern for a date with day, abbreviated month and year (`MMM d, y`, `d. MMM y`).
#[cfg(all(target_os = "macos", not(any(test, feature = "test-util"))))]
fn read() -> Option<String> {
    use objc2_foundation::{NSDateFormatter, NSLocale, NSString};
    let locale = NSLocale::currentLocale();
    NSDateFormatter::dateFormatFromTemplate_options_locale(
        &NSString::from_str("yMMMd"),
        0,
        Some(&locale),
    )
    .map(|pattern| pattern.to_string())
}

/// No locale in tests and on other platforms.
#[cfg(not(all(target_os = "macos", not(any(test, feature = "test-util")))))]
fn read() -> Option<String> {
    None
}

/// Day first when the pattern's day comes before its month; quoted literals (`'de'`) are skipped.
pub(crate) fn order_of(pattern: &str) -> DateOrder {
    let unquoted: String = pattern.split('\'').step_by(2).collect::<Vec<_>>().concat();
    let day = unquoted.find('d');
    let month = unquoted.find(['M', 'L']);
    match (day, month) {
        (Some(d), Some(m)) if d < m => DateOrder::DayFirst,
        _ => DateOrder::MonthFirst,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_reads_day_first_when_the_day_precedes_the_month() {
        for (pattern, expected) in [
            ("MMM d, y", DateOrder::MonthFirst),
            ("d. MMM y", DateOrder::DayFirst),
            ("d MMM y", DateOrder::DayFirst),
            ("d 'de' MMM 'de' y", DateOrder::DayFirst),
            ("y年M月d日", DateOrder::MonthFirst),
        ] {
            assert_eq!(order_of(pattern), expected, "{pattern}");
        }
    }

    #[test]
    fn tests_always_see_month_first() {
        assert_eq!(DateOrder::system(), DateOrder::MonthFirst);
    }
}
