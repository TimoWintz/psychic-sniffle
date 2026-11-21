//! The answer-leak filter: numbers in the tutor's reply that match a secret are replaced.
//!
//! This is a backstop behind the teaching policy, not a proof. It catches digits; it does not catch
//! a number spelled out in words.

use crate::context::{Secret, TutorContext};

pub const REDACTION: &str = "[yours to work out]";
/// Bytes that must follow a number before a streaming reply shows it: enough for the words that
/// decide whether it is a secret ("5 of the blocks", "12 metres per second").
const STREAM_LOOKAHEAD: usize = 24;

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
            if !self.is_secret(&number, &text[number.end..]) {
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

    fn is_secret(&self, number: &Found, following: &str) -> bool {
        let value = number.value;
        if self.known.iter().any(|k| same_number(*k, value)) {
            return false;
        }
        self.secrets.iter().any(|secret| {
            if !(secret.low..=secret.high).contains(&value) {
                return false;
            }
            if number.whole && !secret.whole_needs.is_empty() && !followed_by(following, &secret.whole_needs, 0) {
                return false;
            }
            secret.only_before.is_empty() || followed_by(following, &secret.only_before, secret.gap)
        })
    }
}

/// Whether one of `words` comes next in `text`, as a whole word, after at most `gap` other words.
fn followed_by(text: &str, words: &[String], gap: usize) -> bool {
    let mut rest = text.trim_start();
    for _ in 0..=gap {
        let lower = rest.to_lowercase();
        if words.iter().any(|word| starts_with_word(&lower, &word.to_lowercase())) {
            return true;
        }
        // Step over one word of letters; anything else ends the search.
        let skipped = rest.trim_start_matches(|c: char| c.is_alphabetic() || c == '\'');
        if skipped.len() == rest.len() || !skipped.starts_with(char::is_whitespace) {
            return false;
        }
        rest = skipped.trim_start();
    }
    false
}

/// Whether `text` starts with `word` and the word ends there: "m" starts "m." and "m²" but not
/// "metres" or "m/s".
fn starts_with_word(text: &str, word: &str) -> bool {
    text.strip_prefix(word).is_some_and(|after| !after.starts_with(|c: char| c.is_alphabetic() || c == '/'))
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
    /// Written without a decimal part ("12", not "12.0").
    whole: bool,
}

/// The values of every decimal number in `text`.
pub(crate) fn number_values(text: &str) -> Vec<f64> {
    numbers(text).into_iter().map(|n| n.value).collect()
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
        let whole_end = i;
        // A decimal point (or decimal comma) counts only if a digit follows: "8." ends a sentence.
        if i + 1 < bytes.len() && (bytes[i] == b'.' || bytes[i] == b',') && bytes[i + 1].is_ascii_digit() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
        let literal = text[start..i].replace(',', ".");
        if let Ok(value) = literal.parse::<f64>() {
            found.push(Found { value, start, end: i, whole: i == whole_end });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn narrow(v: f64) -> Secret {
        Secret::near(v, &["m/s"])
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
        let angle = Secret::between(52.0, 64.0).before(&["deg", "degree", "degrees", "°"]);
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
        // The words after it decide, so it waits for them.
        assert_eq!(f.apply_streaming("the speed is 18.43 m/s, so"), "the speed is ");
        assert_eq!(f.apply_streaming("the speed is 18.43 m/s, so check your readings"), format!("the speed is {REDACTION} m/s, so check your readings"));
        assert_eq!(f.apply_streaming("there are 2 steps here, and more to come"), "there are 2 steps here, and more to come");
    }

    #[test]
    fn a_unit_counts_as_a_whole_word_wherever_the_sentence_ends() {
        let height = Secret::between(7.8, 8.2).before(&["m", "metre", "metres"]);
        let f = LeakFilter::new(vec![height], vec![]);
        for line in ["Release from 8 m", "(H = 8 m)", "H = 8 m?", "8 m.", "8 m high", "8 metres", "8m up"] {
            assert!(f.apply(line).contains(REDACTION), "{line}");
        }
        // A speed or a word that only starts with the unit is something else.
        for line in ["8 m/s", "8 minutes", "8 mass units"] {
            assert_eq!(f.apply(line), line);
        }
    }

    #[test]
    fn a_counting_noun_may_come_a_few_words_later() {
        let stack = Secret::between(4.99, 5.01).before(&["block", "blocks"]).gap(2);
        let f = LeakFilter::new(vec![stack], vec![]);
        for line in ["Use 5 blocks.", "Use 5 stone blocks.", "Use 5 of the blocks.", "Use 5 more dressed blocks."] {
            assert!(f.apply(line).contains(REDACTION), "{line}");
        }
        for line in ["Count 5 seconds, then look at the blocks.", "5 m from the blocks", "It took 5 tries, then the blocks"] {
            assert_eq!(f.apply(line), line);
        }
    }

    #[test]
    fn a_whole_number_needs_its_unit_when_the_answer_is_measured() {
        // The answer is close to 2 and to 3: formulas and counts still read normally.
        for answer in [2.03, 2.97] {
            let f = LeakFilter::new(vec![narrow(answer)], vec![]);
            let line = "Use h = (1/2) g t^2, the 2 timing posts and your 3 charges.";
            assert_eq!(f.apply(line), line, "{answer}");
            let rounded = format!("about {} m/s", answer.round());
            assert!(f.apply(&rounded).contains(REDACTION), "{rounded}");
            assert!(f.apply(&format!("about {answer}")).contains(REDACTION), "{answer}");
        }
        // A counted answer is guarded wherever it stands.
        let f = LeakFilter::new(vec![Secret::near(12.0, &[])], vec![]);
        assert_eq!(f.apply("send it 12 along"), format!("send it {REDACTION} along"));
    }
}
