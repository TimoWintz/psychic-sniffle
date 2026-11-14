//! Lumen's one gesture: turning its gaze to a named place.
//!
//! A reply may contain `[look: NAME]`. The tag is cut out before the text is shown, and NAME counts
//! only if it is one of the places the game listed in the snapshot. The game moves Lumen's eye and
//! nothing else, so the tag cannot act in the world; and because the names come from a closed list
//! it cannot carry text around the leak filter either.

/// Remove every look tag from `text`. Returns the text without tags and the names, in order, that
/// match one of `landmarks` (returned as spelled in `landmarks`). Unknown names are dropped.
pub fn strip_looks(text: &str, landmarks: &[String]) -> (String, Vec<String>) {
    let mut out = String::with_capacity(text.len());
    let mut looks = Vec::new();
    let mut rest = text;
    while let Some((start, name_start)) = find_open(rest) {
        let Some(close) = rest[name_start..].find(']').map(|c| name_start + c) else { break };
        out.push_str(&rest[..start]);
        let name = rest[name_start..close].trim();
        if let Some(known) = landmarks.iter().find(|l| l.trim().eq_ignore_ascii_case(name)) {
            looks.push(known.clone());
        }
        rest = &rest[close + 1..];
        // A tag between two spaces leaves one space, not two; a tag before punctuation leaves none.
        if (out.is_empty() || out.ends_with(' ') || out.ends_with('\n')) && rest.starts_with(' ') {
            rest = &rest[1..];
        } else if out.ends_with(' ') && rest.starts_with(['.', ',', ';', ':', '!', '?']) {
            out.pop();
        }
    }
    out.push_str(rest);
    (out, looks)
}

/// As [`strip_looks`], for a reply still arriving: a tag that has begun but not closed is held
/// back, and so is a trailing `[` that may still become one.
pub fn strip_looks_streaming(text: &str, landmarks: &[String]) -> (String, Vec<String>) {
    let (clean, looks) = strip_looks(text, landmarks);
    let cut = match clean.rfind('[') {
        Some(at) if could_open(&clean[at..]) => at,
        _ => clean.len(),
    };
    (clean[..cut].to_string(), looks)
}

/// The next tag: where its `[` is and where its name begins. Case is ignored, and spaces are allowed
/// inside the bracket ("[ Look :").
fn find_open(text: &str) -> Option<(usize, usize)> {
    let mut from = 0;
    while let Some(offset) = text[from..].find('[') {
        let at = from + offset;
        if let Some(name_start) = opening_length(&text[at..]) {
            return Some((at, at + name_start));
        }
        from = at + 1;
    }
    None
}

/// If `text` starts with a complete tag opening, its length.
fn opening_length(text: &str) -> Option<usize> {
    let after_bracket = text.strip_prefix('[')?;
    let word = after_bracket.trim_start();
    if !word.get(..4)?.eq_ignore_ascii_case("look") {
        return None;
    }
    let colon = word[4..].trim_start().strip_prefix(':')?;
    Some(text.len() - colon.len())
}

/// Whether `text` (which starts with `[` and runs to the end of what has arrived) may still turn
/// into a tag.
fn could_open(text: &str) -> bool {
    if text.contains(']') {
        return false;
    }
    let word = text[1..].trim_start().to_ascii_lowercase();
    // Either the opening is complete (the name is still arriving) or what is there is its prefix.
    opening_length(text).is_some() || {
        let head: String = word.chars().filter(|c| !c.is_whitespace()).collect();
        "look:".starts_with(&head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn places() -> Vec<String> {
        ["Platform: centre", "Ledge launcher: release point", "The wreck"].map(String::from).to_vec()
    }

    #[test]
    fn a_tag_is_removed_and_its_place_returned() {
        let (text, looks) = strip_looks("See the target [look: Platform: centre] over there.", &places());
        assert_eq!(text, "See the target over there.");
        assert_eq!(looks, vec!["Platform: centre"]);
    }

    #[test]
    fn spelling_and_spacing_are_forgiven_but_the_list_spelling_is_returned() {
        let (text, looks) = strip_looks("[ LOOK :  platform: CENTRE ] Measure from here.", &places());
        assert_eq!(text, "Measure from here.");
        assert_eq!(looks, vec!["Platform: centre"]);
    }

    #[test]
    fn an_unknown_place_is_dropped_and_still_hidden() {
        let (text, looks) = strip_looks("Look [look: the answer is 12 m/s] here.", &places());
        assert_eq!(text, "Look here.");
        assert!(looks.is_empty());
    }

    #[test]
    fn several_tags_come_back_in_order() {
        let (text, looks) = strip_looks("From [look: Ledge launcher: release point] to [look: Platform: centre].", &places());
        assert_eq!(text, "From to.");
        assert_eq!(looks, vec!["Ledge launcher: release point", "Platform: centre"]);
    }

    #[test]
    fn ordinary_brackets_are_left_alone() {
        let (text, looks) = strip_looks("Use [h] = m and [looking] glass.", &places());
        assert_eq!(text, "Use [h] = m and [looking] glass.");
        assert!(looks.is_empty());
    }

    #[test]
    fn a_tag_still_arriving_is_held_back() {
        for partial in ["Over there [", "Over there [lo", "Over there [look", "Over there [look:", "Over there [look: The wr"] {
            let (text, looks) = strip_looks_streaming(partial, &places());
            assert_eq!(text, "Over there ", "{partial}");
            assert!(looks.is_empty());
        }
        let (text, looks) = strip_looks_streaming("Over there [look: The wreck] is", &places());
        assert_eq!(text, "Over there is");
        assert_eq!(looks, vec!["The wreck"]);
        // A bracket that cannot become a tag shows at once.
        assert_eq!(strip_looks_streaming("units [m", &places()).0, "units [m");
    }
}
