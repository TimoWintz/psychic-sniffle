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
    /// Instrument or origin: "ruler", "stopwatch", "calculated".
    pub source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Notebook {
    pub entries: Vec<Entry>,
}

impl Notebook {
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
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
        let name = (1..)
            .map(|n| format!("{prefix}{n}"))
            .find(|candidate| self.get(candidate).is_none())
            .expect("an unused name exists");
        self.set(&name, label, quantity, source);
        name
    }

    pub fn remove(&mut self, name: &str) {
        self.entries.retain(|e| e.name != name);
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
    fn set_replaces() {
        let mut nb = Notebook::default();
        nb.set("g", "gravity", Quantity::acceleration(8.0), "calculated");
        nb.set("g", "gravity", Quantity::acceleration(8.1), "calculated");
        assert_eq!(nb.entries.len(), 1);
        assert_eq!(nb.get("g").unwrap().quantity.value, 8.1);
    }
}
