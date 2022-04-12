use std::num::NonZeroU16;

#[derive(Debug, PartialEq)]
pub struct SecretNumber(NonZeroU16);

impl SecretNumber {
    pub fn parse<T: AsRef<str>>(string: T) -> Option<Self> {
        let number: u16 = string.as_ref().parse().ok()?;

        if number > 987 || number < 12 {
            return None;
        }

        let units = number % 10;
        let tens = (number / 10) % 10;
        let hundreds = (number / 100) % 10;

        // The digits of the number must be unique.
        if !(units != tens && units != hundreds && hundreds != tens) {
            return None;
        }

        // SAFETY: The number is guaranteed to be greater than zero above.
        unsafe { Some(SecretNumber(NonZeroU16::new_unchecked(number))) }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn rejects_invalid_string() {
        assert_eq!(SecretNumber::parse("12a"), None);
        assert_eq!(SecretNumber::parse("as./,dkgfja.k/lsdjg"), None);
        assert_eq!(SecretNumber::parse("ab2"), None);
        assert_eq!(SecretNumber::parse(""), None);
        assert_eq!(SecretNumber::parse("879a"), None);
    }

    #[test]
    fn rejects_invalid_u16() {
        assert_eq!(SecretNumber::parse("65536"), None);
        assert_eq!(SecretNumber::parse("-1"), None);
        assert_eq!(SecretNumber::parse("1293819023809123890"), None);
    }

    #[test]
    fn rejects_invalid_range_of_numbers() {
        assert_eq!(SecretNumber::parse("1000"), None);
        assert_eq!(SecretNumber::parse("988"), None);
        assert_eq!(SecretNumber::parse("11"), None);
        assert_eq!(SecretNumber::parse("38252"), None);
        assert_eq!(SecretNumber::parse("3128"), None);
    }

    #[test]
    fn rejects_non_unique_numbers() {
        assert_eq!(SecretNumber::parse("22"), None);
        assert_eq!(SecretNumber::parse("101"), None);
        assert_eq!(SecretNumber::parse("199"), None);
        assert_eq!(SecretNumber::parse("911"), None);
        assert_eq!(SecretNumber::parse("666"), None);
        assert_eq!(SecretNumber::parse("383"), None);
        assert_eq!(SecretNumber::parse("311"), None);
        assert_eq!(SecretNumber::parse("339"), None);
        assert_eq!(SecretNumber::parse("155"), None);
        assert_eq!(SecretNumber::parse("112"), None);
        assert_eq!(SecretNumber::parse("99"), None);
    }

    #[test]
    fn parses_unique_three_digit_numbers() {
        assert_eq!(SecretNumber::parse("123").unwrap().0.get(), 123);
        assert_eq!(SecretNumber::parse("152").unwrap().0.get(), 152);
        assert_eq!(SecretNumber::parse("921").unwrap().0.get(), 921);
        assert_eq!(SecretNumber::parse("756").unwrap().0.get(), 756);
        assert_eq!(SecretNumber::parse("987").unwrap().0.get(), 987);
        assert_eq!(SecretNumber::parse("012").unwrap().0.get(), 12);
        assert_eq!(SecretNumber::parse("019").unwrap().0.get(), 19);
        assert_eq!(SecretNumber::parse("536").unwrap().0.get(), 536);
        assert_eq!(SecretNumber::parse("671").unwrap().0.get(), 671);
    }
}
