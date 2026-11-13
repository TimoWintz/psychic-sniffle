//! The answer-leak filter: numbers in the tutor's reply that match a secret are replaced.
//!
//! This is a backstop behind the teaching policy, not a proof. It catches digits; it does not catch
//! a number spelled out in words.

use crate::context::{Secret, TutorContext};

pub const REDACTION: &str = "[yours to work out]";
/// Bytes that must follow a number before a streaming reply shows it.
const STREAM_LOOKAHEAD: usize = 8;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LeakFilter {
    secrets: Vec<Secret>,
    /// Numbers the player already has. An exact match is never redacted.
    known: Vec<f64>,
}

impl LeakFilter {
    pub fn new(secrets: Vec<Secret>, known: Vec<f64>) -> Self {
        LeakFilter { secrets, known }
    }

    /// Build from a snapshot. `player_text` is everything the player has typed in the chat: numbers
    /// they wrote themselves are theirs, and echoing them is not a leak.
    pub fn for_context(ctx: &TutorContext, player_text: &str) -> Self {
        let mut known = ctx.known_numbers.clone();
        known.extend(numbers(player_text).into_iter().map(|n| n.value));
        LeakFilter { secrets: ctx.secrets.clone(), known }
    }

    /// Redact a complete reply.
    pub fn apply(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut copied_to = 0;
        for number in numbers(text) {
            if !self.is_secret(number.value, &text[number.end..]) {
                continue;
            }
            out.push_str(&text[copied_to..number.start]);
            out.push_str(REDACTION);
            copied_to = number.end;
        }
        out.push_str(&text[copied_to..]);
        out
    }

    /// Redact a reply that is still arriving. A number at the very end may be incomplete, and the
    /// unit that decides whether it is a secret may not have arrived, so it is held back until
    /// [`STREAM_LOOKAHEAD`] bytes follow it.
    pub fn apply_streaming(&self, text: &str) -> String {
        let cut = match numbers(text).last() {
            Some(last) if text.len() - last.end < STREAM_LOOKAHEAD => last.start,
            _ => text.len(),
        };
        self.apply(&text[..cut])
    }

    fn is_secret(&self, value: f64, following: &str) -> bool {
        if self.known.iter().any(|k| same_number(*k, value)) {
            return false;
        }
        self.secrets.iter().any(|secret| {
            if !(secret.low..=secret.high).contains(&value) {
                return false;
            }
            if secret.only_before.is_empty() {
                return true;
            }
            let rest = following.trim_start().to_lowercase();
            secret.only_before.iter().any(|unit| rest.starts_with(&unit.to_lowercase()))
        })
    }
}

/// Whether a number written in text is the same as a stored value, allowing for the rounding of the
/// written form (four significant figures).
fn same_number(stored: f64, written: f64) -> bool {
    (stored - written).abs() <= 5e-4 * stored.abs().max(1e-12)
}

struct Found {
    value: f64,
    start: usize,
    end: usize,
}

/// Every decimal number in `text`, with its byte range. Signs are ignored.
fn numbers(text: &str) -> Vec<Found> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        // A decimal point (or decimal comma) counts only if a digit follows: "8." ends a sentence.
        if i + 1 < bytes.len() && (bytes[i] == b'.' || bytes[i] == b',') && bytes[i + 1].is_ascii_digit() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
        let literal = text[start..i].replace(',', ".");
        if let Ok(value) = literal.parse::<f64>() {
            found.push(Found { value, start, end: i });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn narrow(v: f64) -> Secret {
        Secret { low: v * 0.98, high: v * 1.02, only_before: Vec::new() }
    }

    #[test]
    fn redacts_the_answer_and_near_misses() {
        let f = LeakFilter::new(vec![narrow(8.43)], vec![]);
        assert_eq!(f.apply("g is 8.43 m/s^2."), format!("g is {REDACTION} m/s^2."));
        assert_eq!(f.apply("about 8.4, I think"), format!("about {REDACTION}, I think"));
        assert_eq!(f.apply("roughly 8,43"), format!("roughly {REDACTION}"));
        assert_eq!(f.apply("try h = (1/2) g t^2 with t = 2.18 s"), "try h = (1/2) g t^2 with t = 2.18 s");
        assert_eq!(f.apply("no numbers here"), "no numbers here");
    }

    #[test]
    fn what_the_player_already_has_is_not_redacted() {
        // The player's own reading happens to be the answer to four figures.
        let f = LeakFilter::new(vec![narrow(8.43)], vec![8.4312]);
        assert_eq!(f.apply("your g1 = 8.431 looks right to test"), "your g1 = 8.431 looks right to test");
        // ...but a different nearby number is still caught.
        assert_eq!(f.apply("it is 8.40"), format!("it is {REDACTION}"));
    }

    #[test]
    fn wide_ranges_need_their_unit() {
        let angle = Secret { low: 52.0, high: 64.0, only_before: vec!["deg".into(), "°".into()] };
        let f = LeakFilter::new(vec![angle], vec![]);
        assert_eq!(f.apply("aim at 58 degrees"), format!("aim at {REDACTION} degrees"));
        assert_eq!(f.apply("aim at 58°"), format!("aim at {REDACTION}°"));
        assert_eq!(f.apply("the pad is 58 m away"), "the pad is 58 m away");
        assert_eq!(f.apply("45 degrees gives the longest range"), "45 degrees gives the longest range");
    }

    #[test]
    fn streaming_holds_back_an_unfinished_number() {
        let f = LeakFilter::new(vec![narrow(18.43)], vec![]);
        // "18" alone is not the answer, but it may be the start of it.
        assert_eq!(f.apply_streaming("the speed is 18"), "the speed is ");
        assert_eq!(f.apply_streaming("the speed is 18.4"), "the speed is ");
        assert_eq!(f.apply_streaming("the speed is 18.43 m/s, so"), format!("the speed is {REDACTION} m/s, so"));
        assert_eq!(f.apply_streaming("there are 2 steps here"), "there are 2 steps here");
    }
}
