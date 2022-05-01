use std::num::NonZeroU16;

#[derive(Debug, PartialEq)]
pub struct Secret(NonZeroU16);

impl Secret {
    pub fn get(&self) -> u16 {
        self.0.get()
    }

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
        unsafe { Some(Secret(NonZeroU16::new_unchecked(number))) }
    }

    pub fn score(&self, guess: &Secret) -> (u8, u8) {
        let secret = self.get();
        let guess = guess.get();

        let secret = [(secret / 100) % 10, (secret / 10) % 10, secret % 10];
        let guess = [(guess / 100) % 10, (guess / 10) % 10, guess % 10];

        let mut correct_position = 0;
        let mut wrong_position = 0;

        for i in 0..3 {
            if secret[i] == guess[i] {
                correct_position += 1;
            } else if secret[i % 3] == guess[(i + 1) % 3] || secret[i % 3] == guess[(i + 2) % 3] {
                wrong_position += 1;
            }
        }

        (correct_position, wrong_position)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn rejects_invalid_string() {
        assert_eq!(Secret::parse("12a"), None);
        assert_eq!(Secret::parse("as./,dkgfja.k/lsdjg"), None);
        assert_eq!(Secret::parse("ab2"), None);
        assert_eq!(Secret::parse(""), None);
        assert_eq!(Secret::parse("879a"), None);
    }

    #[test]
    fn rejects_invalid_u16() {
        assert_eq!(Secret::parse("65536"), None);
        assert_eq!(Secret::parse("-1"), None);
        assert_eq!(Secret::parse("1293819023809123890"), None);
    }

    #[test]
    fn rejects_invalid_range_of_numbers() {
        assert_eq!(Secret::parse("1000"), None);
        assert_eq!(Secret::parse("988"), None);
        assert_eq!(Secret::parse("11"), None);
        assert_eq!(Secret::parse("38252"), None);
        assert_eq!(Secret::parse("3128"), None);
    }

    #[test]
    fn rejects_non_unique_numbers() {
        assert_eq!(Secret::parse("22"), None);
        assert_eq!(Secret::parse("101"), None);
        assert_eq!(Secret::parse("199"), None);
        assert_eq!(Secret::parse("911"), None);
        assert_eq!(Secret::parse("666"), None);
        assert_eq!(Secret::parse("383"), None);
        assert_eq!(Secret::parse("311"), None);
        assert_eq!(Secret::parse("339"), None);
        assert_eq!(Secret::parse("155"), None);
        assert_eq!(Secret::parse("112"), None);
        assert_eq!(Secret::parse("99"), None);
    }

    #[test]
    fn parses_unique_three_digit_numbers() {
        macro_rules! parse_assert_eq {
            ($($secret:literal => $number:literal),*) => {
                $(assert_eq!(Secret::parse($secret).unwrap().0.get(), $number);)*
            }
        }

        parse_assert_eq! {
            "123" => 123,
            "152" => 152,
            "921" => 921,
            "756" => 756,
            "987" => 987,
            "012" => 12,
            "019" => 19,
            "536" => 536,
            "671" => 671
        }
    }

    #[test]
    fn scores_guesses() {
        macro_rules! score_assert_eq {
            ($($secret:literal $guess:literal => $correct:literal $wrong:literal),*) => {
                $(
                    assert_eq!(
                        Secret::parse($secret).unwrap()
                            .score(&Secret::parse($guess).unwrap()),
                        ($correct, $wrong)
                    )
                ;)*
            }
        }

        score_assert_eq! {
            "123" "456" => 0 0,
            "123" "123" => 3 0,
            "123" "312" => 0 3,
            "123" "321" => 1 2,
            "123" "230" => 0 2,
            "123" "923" => 2 0,
            "123" "142" => 1 1
        }
    }
}
