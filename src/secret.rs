use std::num::NonZeroU16;
use std::fmt::{Formatter, Result as FormatResult};
use serde::{
    Serialize,
    de::{Deserialize, Deserializer,Visitor, Error, Unexpected}
};

#[derive(Debug, PartialEq, Serialize)]
pub struct Secret(NonZeroU16);

struct SecretVisitor;

impl<'a> Visitor<'a> for SecretVisitor {
    type Value = Secret;

    fn expecting(&self, formatter: &mut Formatter) -> FormatResult {
        formatter.write_str("a sequence of three unique digits")
    }

    fn visit_u64<E: Error>(self, n: u64) -> Result<Self::Value, E> {
        if 988 > n && n > 10 {
            let units = n % 10;
            let tens = (n / 10) % 10;
            let hundreds = (n / 100) % 10;

            // The digits of the number must be unique.
            if units != tens && units != hundreds && hundreds != tens {
                unsafe {
                    // SAFETY: The number is guaranteed to be less than u16::MAX.
                    let n: u16 = n.try_into().unwrap_unchecked();

                    // SAFETY: The number is guaranteed to be greater than zero in
                    // this scope, because the first if conditon eliminated that
                    // possibility earlier.
                    Ok(Secret(NonZeroU16::new_unchecked(n)))
                }
            } else {
                Err(Error::invalid_value(Unexpected::Unsigned(n), &self))
            }
        } else {
            Err(Error::invalid_value(Unexpected::Unsigned(n), &self))
        }
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u16(SecretVisitor)
    }
}

impl Secret {
    pub fn parse<T: AsRef<str>>(_: T) -> Option<Self> {
        None
    }

    pub fn score(&self, guess: &Secret) -> (u8, u8) {
        let secret = self.0.get();
        let guess = guess.0.get();

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
    use serde_json::{json, from_value};

    macro_rules! rejects {
        ($($number:literal),*) => {
            $(assert!(matches!(from_value::<Secret>(json!($number)), Err(_)));)*
        }
    }

    macro_rules! accepts {
        ($($number:literal),*) => {
            $(assert!(matches!(from_value::<Secret>(json!($number)), Ok(_)));)*
        }
    }

    macro_rules! scores {
        ($($secret:literal $guess:literal => $correct:literal $wrong:literal),*) => {
            $(
                let secret = from_value::<Secret>(json!($secret)).unwrap();
                let guess = from_value::<Secret>(json!($guess)).unwrap();
                assert_eq!(secret.score(&guess), ($correct, $wrong));
             )*
        }
    }

    #[test]
    fn rejects_invalid_range_of_numbers() {
        rejects!(0, 1000, 988, 11, 38252, 3128, 9238, 2381, -1, -1283, -123);
    }

    #[test]
    fn rejects_non_unique_numbers() {
        rejects!(22, 101, 199, 911, 666, 383, 311, 339, 55, 112, 99, 133);
    }

    #[test]
    fn accepts_unique_three_digit_numbers() {
        accepts!(123, 152, 921, 756, 987, 12, 19, 536, 671);
    }

    #[test]
    fn scores_guesses() {
        scores! {
            123 456 => 0 0,
            123 123 => 3 0,
            123 312 => 0 3,
            123 321 => 1 2,
            123 230 => 0 2,
            123 923 => 2 0,
            123 142 => 1 1,
            42 42 => 3 0
        }
    }

}
