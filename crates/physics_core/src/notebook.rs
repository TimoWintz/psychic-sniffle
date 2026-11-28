//! The player's notebook: named readings, which are also the calculator's variables.

use crate::expr::Env;
use crate::units::Quantity;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Short name usable in the calculator: `h1`, `t1`, `g`.
    pub name: String,
    /// What was measured, in words.
    pub label: String,
    pub quantity: Quantity,
    /// Instrument or origin, one of [`source`]'s names: "ruler", "stopwatch", "calculated".
    pub source: String,
}

/// Where a notebook entry came from. Saved as text, so these are the words the save holds.
pub mod source {
    pub const RULER: &str = "ruler";
    pub const STOPWATCH: &str = "stopwatch";
    pub const LIGHT_GATE: &str = "light gate";
    /// The ledge launcher's rating plate.
    pub const PLATE: &str = "plate";
    /// The calculator.
    pub const CALCULATED: &str = "calculated";
    /// The player's own g, from a pass at the drop tower.
    pub const CALIBRATED: &str = "calibrated";
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Notebook {
    pub entries: Vec<Entry>,
}

impl Notebook {
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Whether the notebook already holds this reading: the same label and the same value.
    pub fn has_reading(&self, label: &str, quantity: Quantity) -> bool {
        self.entries.iter().any(|e| e.label == label && e.quantity == quantity)
    }

    /// Record a reading under `name`, replacing an earlier entry with the same name.
    pub fn set(&mut self, name: &str, label: &str, quantity: Quantity, source: &str) {
        let entry = Entry { name: name.into(), label: label.into(), quantity, source: source.into() };
        match self.entries.iter_mut().find(|e| e.name == name) {
            Some(existing) => *existing = entry,
            None => self.entries.push(entry),
        }
    }

    /// Record a reading under the first free name `prefix1`, `prefix2`, ... and return that name.
    pub fn add_numbered(&mut self, prefix: &str, label: &str, quantity: Quantity, source: &str) -> String {
        let name = (1..).map(|n| format!("{prefix}{n}")).find(|candidate| self.get(candidate).is_none()).expect("an unused name exists");
        self.set(&name, label, quantity, source);
        name
    }

    /// Record two readings that belong together under the first number free for both prefixes
    /// (`h2` and `t2`), so a pair keeps one number. Returns the two names.
    pub fn add_pair(&mut self, first: (&str, &str, Quantity), second: (&str, &str, Quantity), source: &str) -> (String, String) {
        let n =
            (1..).find(|n| self.get(&format!("{}{n}", first.0)).is_none() && self.get(&format!("{}{n}", second.0)).is_none()).expect("an unused number exists");
        let names = (format!("{}{n}", first.0), format!("{}{n}", second.0));
        self.set(&names.0, first.1, first.2, source);
        self.set(&names.1, second.1, second.2, source);
        names
    }

    pub fn remove(&mut self, name: &str) {
        self.entries.retain(|e| e.name != name);
    }

    /// Rename `old` to `new` in place. Returns `false` if `old` does not exist or `new` is already
    /// taken by another entry.
    pub fn rename(&mut self, old: &str, new: &str) -> bool {
        if old == new {
            return self.get(old).is_some();
        }
        if self.get(new).is_some() {
            return false;
        }
        let Some(entry) = self.entries.iter_mut().find(|e| e.name == old) else { return false };
        entry.name = new.into();
        true
    }
}

impl Env for Notebook {
    fn var(&self, name: &str) -> Option<Quantity> {
        self.get(name).map(|e| e.quantity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::evaluate;

    #[test]
    fn entries_are_calculator_variables() {
        let mut nb = Notebook::default();
        let h = nb.add_numbered("h", "tower height", Quantity::metres(20.0), "ruler");
        let t = nb.add_numbered("t", "fall time", Quantity::seconds(2.0), "stopwatch");
        assert_eq!((h.as_str(), t.as_str()), ("h1", "t1"));
        let g = evaluate("2*h1/t1^2", &nb).unwrap();
        assert_eq!(g, Quantity::acceleration(10.0));
        assert_eq!(nb.add_numbered("h", "ledge", Quantity::metres(8.0), "ruler"), "h2");
    }

    #[test]
    fn pairs_share_a_number() {
        let mut nb = Notebook::default();
        nb.add_numbered("h", "tower height", Quantity::metres(20.0), "ruler");
        nb.add_numbered("t", "fall time", Quantity::seconds(2.0), "stopwatch");
        let pair = nb.add_pair(("h", "drop to the gate", Quantity::metres(5.0)), ("t", "time to the gate", Quantity::seconds(1.0)), "light gate");
        assert_eq!(pair, ("h2".to_string(), "t2".to_string()));
        nb.add_numbered("t", "fall time", Quantity::seconds(2.0), "stopwatch");
        let pair = nb.add_pair(("h", "drop to the gate", Quantity::metres(9.0)), ("t", "time to the gate", Quantity::seconds(1.4)), "light gate");
        assert_eq!(pair, ("h4".to_string(), "t4".to_string()), "t3 is taken, so the pair skips to 4");
    }

    #[test]
    fn a_reading_is_known_by_its_label_and_value() {
        let mut nb = Notebook::default();
        nb.add_numbered("t", "fall time", Quantity::seconds(2.0), "stopwatch");
        assert!(nb.has_reading("fall time", Quantity::seconds(2.0)));
        assert!(!nb.has_reading("fall time", Quantity::seconds(2.1)));
        assert!(!nb.has_reading("rise time", Quantity::seconds(2.0)));
    }

    #[test]
    fn set_replaces() {
        let mut nb = Notebook::default();
        nb.set("g", "gravity", Quantity::acceleration(8.0), "calculated");
        nb.set("g", "gravity", Quantity::acceleration(8.1), "calculated");
        assert_eq!(nb.entries.len(), 1);
        assert_eq!(nb.get("g").unwrap().quantity.value, 8.1);
    }

    #[test]
    fn rename_keeps_order_and_refuses_collisions() {
        let mut nb = Notebook::default();
        nb.add_numbered("d", "first", Quantity::metres(10.0), "ruler");
        nb.add_numbered("d", "second", Quantity::metres(20.0), "ruler");
        assert!(nb.rename("d1", "span"));
        assert_eq!(nb.entries[0].name, "span");
        assert_eq!(nb.get("span").unwrap().quantity, Quantity::metres(10.0));
        assert!(nb.get("d1").is_none());
        assert!(!nb.rename("span", "d2"), "cannot overwrite another entry");
        assert!(!nb.rename("missing", "other"));
        assert!(nb.rename("span", "span"));
    }
}
