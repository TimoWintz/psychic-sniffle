//! The beam: chapter 1.1's side-machine (see "Side-machines" in the game design, and the curriculum).
//!
//! A long beam of the builders' stone on a pivot, with a seat every metre on each arm. The machine
//! sets a load of its own blocks on one seat of the load arm; the player carries blocks onto any seat
//! of either arm and releases the beam at the console. It holds level when the blocks times seats on
//! one arm match the other's, and nothing else is ever said about how far off it was.
//!
//! Nothing here is typed and nothing costs a charge, so pillar 3 is kept by the cases themselves: the
//! last one can't be reached from the one before by moving a single block, and few of all the
//! arrangements the pile allows balance it. The tests check both for every seed.

use crate::puzzle::Rng;
use serde::{Deserialize, Serialize};

/// Seats on each arm, one metre apart from the pivot out.
pub const SEATS: u32 = 5;
/// The most of the player's blocks one seat holds.
pub const MOST_ON_A_SEAT: u32 = 3;
/// Blocks in the pile beside the beam: all the player can put on it.
pub const PILE: u32 = 7;
/// The cases the machine sets, one after another.
pub const CASES: usize = 3;
/// The most of the machine's own blocks it sets on one seat.
pub const MOST_LOAD: u32 = 4;
/// How much more the last case turns than the one before, counted as blocks times seats: more than
/// any one block moved can change (from the end of one arm to the end of the other).
pub const LAST_STEP: u32 = 2 * SEATS + 1;

/// The machine's own blocks on a seat of the load arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Load {
    /// Seat, counted in metres from the pivot (1 to [`SEATS`]).
    pub seat: u32,
    pub blocks: u32,
}

impl Load {
    /// How hard it turns the beam: blocks times seat.
    pub fn turning(&self) -> u32 {
        self.seat * self.blocks
    }
}

/// One of the beam's two arms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arm {
    /// The west arm, where the machine sets its load.
    Load,
    /// The east arm.
    Far,
}

/// The player's blocks on the beam: how many on each seat of each arm (seat 1 first).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Placement {
    pub load_arm: [u32; SEATS as usize],
    pub far_arm: [u32; SEATS as usize],
}

impl Placement {
    /// From the save's list: the load arm's seats, then the far arm's. Anything missing is empty,
    /// and nothing holds more than a seat can.
    pub fn from_saved(saved: &[u32]) -> Self {
        let mut placement = Placement::default();
        for (i, &n) in saved.iter().take(2 * SEATS as usize).enumerate() {
            let (arm, seat) = if i < SEATS as usize { (Arm::Load, i as u32 + 1) } else { (Arm::Far, i as u32 + 1 - SEATS) };
            placement.set(arm, seat, n.min(MOST_ON_A_SEAT));
        }
        placement
    }

    /// For the save: the load arm's seats, then the far arm's.
    pub fn to_saved(&self) -> Vec<u32> {
        self.load_arm.iter().chain(&self.far_arm).copied().collect()
    }

    fn arm(&self, arm: Arm) -> &[u32; SEATS as usize] {
        match arm {
            Arm::Load => &self.load_arm,
            Arm::Far => &self.far_arm,
        }
    }

    /// Blocks on a seat (1 to [`SEATS`]).
    pub fn get(&self, arm: Arm, seat: u32) -> u32 {
        self.arm(arm)[seat as usize - 1]
    }

    pub fn set(&mut self, arm: Arm, seat: u32, blocks: u32) {
        let seats = match arm {
            Arm::Load => &mut self.load_arm,
            Arm::Far => &mut self.far_arm,
        };
        seats[seat as usize - 1] = blocks;
    }

    /// Blocks on the beam.
    pub fn blocks(&self) -> u32 {
        self.load_arm.iter().chain(&self.far_arm).sum()
    }

    /// How hard the blocks turn the beam towards the far end: the far arm's blocks times seats, less
    /// the load arm's.
    fn turning(&self) -> i32 {
        let arm = |seats: &[u32; SEATS as usize]| seats.iter().enumerate().map(|(i, n)| (i as i32 + 1) * *n as i32).sum::<i32>();
        arm(&self.far_arm) - arm(&self.load_arm)
    }
}

/// What the beam did when it was released.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Swing {
    Level,
    /// The load's end went down.
    LoadDown,
    /// The far end went down.
    FarDown,
}

impl Swing {
    pub fn holds(self) -> bool {
        self == Swing::Level
    }

    /// The console's line for it. Which way, never how far.
    pub fn line(self) -> &'static str {
        match self {
            Swing::Level => "The beam held level.",
            Swing::LoadDown => "The load's end went down.",
            Swing::FarDown => "The far end went down.",
        }
    }

    /// The beam's tilt as it rests: +1 with the far end down, −1 with the load's end down.
    pub fn tilt(self) -> f32 {
        match self {
            Swing::Level => 0.0,
            Swing::LoadDown => -1.0,
            Swing::FarDown => 1.0,
        }
    }
}

/// Release the beam with `load` on it and the player's blocks as `placement`.
pub fn release(load: Load, placement: &Placement) -> Swing {
    match placement.turning().cmp(&(load.turning() as i32)) {
        std::cmp::Ordering::Equal => Swing::Level,
        std::cmp::Ordering::Less => Swing::LoadDown,
        std::cmp::Ordering::Greater => Swing::FarDown,
    }
}

/// The three loads the machine sets in a save, in order. They come from their own stream of numbers,
/// so the chapter's layout keeps every one of its draws.
pub fn cases(seed: u64) -> [Load; CASES] {
    let mut rng = Rng::new(seed ^ 0xBEA4_5EA7_0000_0001);
    let first = Load { seat: rng.int(2, 3), blocks: 1 };
    let second = Load { seat: rng.int(2, 3), blocks: 2 };
    let lasts: Vec<Load> = [Load { seat: 5, blocks: 3 }, Load { seat: 4, blocks: 4 }, Load { seat: 5, blocks: 4 }]
        .into_iter()
        .filter(|last| last.turning() >= second.turning() + LAST_STEP)
        .collect();
    let last = lasts[rng.int(0, lasts.len() as u32 - 1) as usize];
    [first, second, last]
}

/// The load the beam is released against once `held` cases have held: the next case, or the last
/// one again once all have.
pub fn current(cases: &[Load; CASES], held: u32) -> Load {
    cases[(held as usize).min(CASES - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEEDS: u64 = 1000;

    /// Everything the pile and the seats allow on the beam.
    fn every_placement() -> Vec<Placement> {
        let cells = 2 * SEATS as usize;
        let base = MOST_ON_A_SEAT + 1;
        (0..base.pow(cells as u32))
            .filter_map(|mut code| {
                let mut counts = Vec::with_capacity(cells);
                for _ in 0..cells {
                    counts.push(code % base);
                    code /= base;
                }
                let placement = Placement::from_saved(&counts);
                (placement.blocks() <= PILE).then_some(placement)
            })
            .collect()
    }

    /// What one move makes of a placement: a block put on from the pile, taken off, or carried from
    /// one seat to another.
    fn one_move(placement: &Placement) -> Vec<Placement> {
        let seats: Vec<(Arm, u32)> = [Arm::Load, Arm::Far].into_iter().flat_map(|arm| (1..=SEATS).map(move |s| (arm, s))).collect();
        let mut next = Vec::new();
        for &(arm, seat) in &seats {
            let n = placement.get(arm, seat);
            if n < MOST_ON_A_SEAT && placement.blocks() < PILE {
                let mut p = *placement;
                p.set(arm, seat, n + 1);
                next.push(p);
            }
            if n > 0 {
                let mut p = *placement;
                p.set(arm, seat, n - 1);
                next.push(p);
                for &(to_arm, to_seat) in &seats {
                    let m = p.get(to_arm, to_seat);
                    if (to_arm, to_seat) != (arm, seat) && m < MOST_ON_A_SEAT {
                        let mut q = p;
                        q.set(to_arm, to_seat, m + 1);
                        next.push(q);
                    }
                }
            }
        }
        next
    }

    #[test]
    fn it_holds_when_blocks_times_seats_match() {
        let load = Load { seat: 2, blocks: 2 };
        let mut placement = Placement::default();
        assert_eq!(release(load, &placement), Swing::LoadDown);
        placement.set(Arm::Far, 4, 1);
        assert_eq!(release(load, &placement), Swing::Level);
        placement.set(Arm::Far, 1, 1);
        assert_eq!(release(load, &placement), Swing::FarDown);
        // A block on the load arm turns with the load.
        placement.set(Arm::Load, 1, 1);
        assert_eq!(release(load, &placement), Swing::Level);
        assert!(Swing::Level.holds() && !Swing::FarDown.holds());
    }

    #[test]
    fn a_placement_round_trips_through_the_save() {
        let mut placement = Placement::default();
        placement.set(Arm::Load, 5, 2);
        placement.set(Arm::Far, 3, 3);
        assert_eq!(Placement::from_saved(&placement.to_saved()), placement);
        assert_eq!(placement.blocks(), 5);
        // A short or damaged list keeps what it can.
        assert_eq!(Placement::from_saved(&[]), Placement::default());
        assert_eq!(Placement::from_saved(&[9]).get(Arm::Load, 1), MOST_ON_A_SEAT);
    }

    #[test]
    fn the_cases_grow_and_every_one_can_be_balanced() {
        let all = every_placement();
        let mut loads = std::collections::BTreeSet::new();
        for seed in 0..SEEDS {
            let [first, second, last] = cases(seed);
            assert_eq!((first.blocks, second.blocks), (1, 2), "seed {seed}");
            assert!(first.turning() <= 3 && second.turning() <= 6, "seed {seed}: the first two can be found by trying");
            assert!(last.blocks >= 3 && last.seat >= 4 && last.blocks <= MOST_LOAD, "seed {seed}: the last is big and far out");
            assert!(last.turning() >= second.turning() + LAST_STEP, "seed {seed}");
            for load in [first, second, last] {
                assert!(load.seat <= SEATS, "seed {seed}");
                loads.insert((load.seat, load.blocks));
            }
        }
        for (seat, blocks) in loads {
            let load = Load { seat, blocks };
            assert!(all.iter().any(|p| release(load, p).holds()), "{load:?} can be balanced");
        }
        // The seed decides: not every save has the same beam.
        let distinct: std::collections::HashSet<_> = (0..SEEDS).map(|s| format!("{:?}", cases(s))).collect();
        assert!(distinct.len() >= 6, "{}", distinct.len());
    }

    #[test]
    fn the_last_case_is_not_one_move_from_the_one_before() {
        let all = every_placement();
        // Each pair of cases once: the seeds draw from a handful.
        let pairs: std::collections::BTreeSet<(u32, u32, u32, u32)> = (0..SEEDS).map(cases).map(|[_, b, c]| (b.seat, b.blocks, c.seat, c.blocks)).collect();
        for (seat, blocks, last_seat, last_blocks) in pairs {
            let (second, last) = (Load { seat, blocks }, Load { seat: last_seat, blocks: last_blocks });
            for answer in all.iter().filter(|p| release(second, p).holds()) {
                for next in one_move(answer) {
                    assert!(!release(last, &next).holds(), "{answer:?} to {next:?} balances {last:?} in one move");
                }
            }
        }
    }

    #[test]
    fn few_of_the_arrangements_balance_the_last_case() {
        // Fiddling finds the last case slowly: few of the arrangements the pile allows balance it,
        // though there are several, so it is a rule to find and not one placement to look up.
        let all = every_placement();
        for last in [Load { seat: 5, blocks: 3 }, Load { seat: 4, blocks: 4 }, Load { seat: 5, blocks: 4 }] {
            let right = all.iter().filter(|p| release(last, p).holds()).count();
            let share = right as f64 / all.len() as f64;
            assert!(right >= 5, "{last:?}: {right} balance it");
            assert!(share < 0.02, "{last:?}: {right} of {} balance it ({:.1} %)", all.len(), share * 100.0);
            // Most of them tip, and the beam says which way: the far end goes down for few of the
            // arrangements, the load's end for most.
            let load_down = all.iter().filter(|p| release(last, p) == Swing::LoadDown).count();
            assert!(load_down * 2 > all.len(), "{last:?}");
        }
    }

    #[test]
    fn after_the_last_case_the_beam_keeps_its_load() {
        let cases = cases(4);
        assert_eq!(current(&cases, 0), cases[0]);
        assert_eq!(current(&cases, 2), cases[2]);
        assert_eq!(current(&cases, 3), cases[2]);
        assert_eq!(current(&cases, 9), cases[2]);
    }
}
