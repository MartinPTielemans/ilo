//! What a sum typed in the command bar comes to, which the bar shows as a row and ↩ on it copies,
//! as Dia's does (docs/ux-spec.md › Command bar).

/// What `text` comes to, if it's arithmetic with an operator in it, such as "12*7" or "(1+2)^3",
/// as the bar shows and copies it. A leading = is allowed.
pub fn calculate(text: &str) -> Option<String> {
    let text = text.trim();
    let text = text.strip_prefix('=').unwrap_or(text);
    if is_date(text) {
        return None;
    }
    let text = text
        .replace("**", "^")
        .replace(['×', '·'], "*")
        .replace('÷', "/")
        .replace('−', "-");
    let mut parser = Parser {
        chars: text.chars().collect(),
        at: 0,
        operators: 0,
    };
    let value = parser.sum()?;
    let done = parser.peek().is_none();
    (done && parser.operators > 0 && value.is_finite()).then(|| format(value))
}

/// 2026-10-26 is a date, not a subtraction.
fn is_date(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    let digits = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
    parts.len() == 3 && parts[0].len() == 4 && parts.iter().all(|part| digits(part))
}

/// 12 significant digits, as a calculator shows them, so 0.1+0.2 is 0.3, and an exponent for
/// numbers too large or small to read.
fn format(value: f64) -> String {
    let value: f64 = format!("{value:.11e}").parse().unwrap_or(value);
    if value == 0. {
        // Not -0.
        return "0".into();
    }
    match (1e-6..1e15).contains(&value.abs()) {
        true => format!("{value}"),
        false => format!("{value:e}"),
    }
}

struct Parser {
    chars: Vec<char>,
    at: usize,
    /// The operators read, so a number alone isn't a calculation.
    operators: usize,
}

impl Parser {
    /// The next character that isn't a space.
    fn peek(&mut self) -> Option<char> {
        while self.chars.get(self.at).is_some_and(|c| c.is_whitespace()) {
            self.at += 1;
        }
        self.chars.get(self.at).copied()
    }

    /// Takes `operator` if it's next.
    fn take(&mut self, operator: char) -> bool {
        let next = self.peek() == Some(operator);
        if next {
            self.at += 1;
        }
        next
    }

    fn sum(&mut self) -> Option<f64> {
        let mut value = self.product()?;
        loop {
            if self.take('+') {
                value += self.product()?;
            } else if self.take('-') {
                value -= self.product()?;
            } else {
                return Some(value);
            }
            self.operators += 1;
        }
    }

    fn product(&mut self) -> Option<f64> {
        let mut value = self.unary()?;
        loop {
            if self.take('*') {
                value *= self.unary()?;
            } else if self.take('/') {
                value /= self.unary()?;
            } else {
                return Some(value);
            }
            self.operators += 1;
        }
    }

    /// A sign binds looser than ^, so -2^2 is -4.
    fn unary(&mut self) -> Option<f64> {
        if self.take('-') {
            return Some(-self.unary()?);
        }
        if self.take('+') {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Option<f64> {
        let base = self.atom()?;
        if !self.take('^') {
            return Some(base);
        }
        self.operators += 1;
        Some(base.powf(self.unary()?))
    }

    fn atom(&mut self) -> Option<f64> {
        if self.take('(') {
            let value = self.sum()?;
            return self.take(')').then_some(value);
        }
        self.peek()?;
        let start = self.at;
        let mut point = false;
        while let Some(&c) = self.chars.get(self.at) {
            match c {
                '0'..='9' => {}
                '.' if !point => point = true,
                _ => break,
            }
            self.at += 1;
        }
        let number: String = self.chars[start..self.at].iter().collect();
        number.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::calculate;

    #[test]
    fn sums_come_to_what_a_calculator_shows() {
        let cases = [
            ("12*7", "84"),
            ("12 × 7", "84"),
            ("= 1 + 2 * 3", "7"),
            ("(1+2)^3", "27"),
            ("2**10", "1024"),
            ("-2^2", "-4"),
            ("2^-1", "0.5"),
            ("0.1+0.2", "0.3"),
            ("1/3", "0.333333333333"),
            ("10 ÷ 4", "2.5"),
            ("2^60", "1.15292150461e18"),
        ];
        for (text, result) in cases {
            assert_eq!(calculate(text).unwrap_or_default(), result, "{text}");
        }
    }

    #[test]
    fn what_isnt_a_sum_has_no_result() {
        let texts = [
            "42",
            "-5",
            "(5)",
            "1.2.3",
            "1e3",
            "2026-10-26",
            "1/0",
            "2+",
            "12 7",
            "a+1",
        ];
        for text in texts {
            assert_eq!(calculate(text), None, "{text}");
        }
    }
}
