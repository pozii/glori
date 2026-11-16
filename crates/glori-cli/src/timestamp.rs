pub fn date(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(951_782_400), "2000-02-29");
        assert_eq!(date(1_700_000_000), "2023-11-14");
        assert_eq!(date(1_763_319_600), "2025-11-16");
    }

    #[test]
    fn end_of_year_and_leap_boundaries() {
        assert_eq!(date(1_709_164_799), "2024-02-28");
        assert_eq!(date(1_709_164_800), "2024-02-29");
        assert_eq!(date(1_709_251_200), "2024-03-01");
        assert_eq!(date(1_735_689_599), "2024-12-31");
        assert_eq!(date(1_735_689_600), "2025-01-01");
    }
}
