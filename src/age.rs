use std::fmt;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgeThreshold {
    days: u64,
}

impl AgeThreshold {
    pub const fn days(days: u64) -> Self {
        Self { days }
    }

    #[cfg(test)]
    pub const fn as_days(self) -> u64 {
        self.days
    }

    pub const fn as_duration(self) -> Duration {
        Duration::from_secs(self.days * 24 * 60 * 60)
    }

    pub const fn presets() -> [Self; 4] {
        [
            Self::days(7),
            Self::days(30),
            Self::days(90),
            Self::days(365),
        ]
    }

    pub fn label(self) -> String {
        match self.days {
            7 => "7d".to_string(),
            30 => "30d".to_string(),
            90 => "90d".to_string(),
            365 => "1y".to_string(),
            days => format!("{days}d"),
        }
    }
}

impl fmt::Display for AgeThreshold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

impl FromStr for AgeThreshold {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let raw = raw.trim().to_ascii_lowercase();
        if raw.is_empty() {
            return Err("age must not be empty".to_string());
        }

        let split_at = raw
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(raw.len());
        let (number, unit) = raw.split_at(split_at);
        let value: u64 = number
            .parse()
            .map_err(|_| "age must start with a positive number".to_string())?;

        if value == 0 {
            return Err("age must be greater than zero".to_string());
        }

        let days = match unit {
            "" | "d" | "day" | "days" => value,
            "w" | "week" | "weeks" => value * 7,
            "m" | "mo" | "month" | "months" => value * 30,
            "y" | "yr" | "year" | "years" => value * 365,
            _ => return Err("age unit must be d, w, m, or y".to_string()),
        };

        Ok(Self::days(days))
    }
}

#[cfg(test)]
mod tests {
    use super::AgeThreshold;

    #[test]
    fn parses_supported_units() {
        assert_eq!("7d".parse::<AgeThreshold>().unwrap().as_days(), 7);
        assert_eq!("2w".parse::<AgeThreshold>().unwrap().as_days(), 14);
        assert_eq!("3m".parse::<AgeThreshold>().unwrap().as_days(), 90);
        assert_eq!("1y".parse::<AgeThreshold>().unwrap().as_days(), 365);
        assert_eq!("45".parse::<AgeThreshold>().unwrap().as_days(), 45);
    }

    #[test]
    fn rejects_invalid_values() {
        assert!("0d".parse::<AgeThreshold>().is_err());
        assert!("soon".parse::<AgeThreshold>().is_err());
        assert!("10q".parse::<AgeThreshold>().is_err());
    }
}
