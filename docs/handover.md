# Handover: open issues from the November 2026 review

A review of the whole repository found the issues below. The six most serious ones are fixed on
`claude/confident-ptolemy-hz598f` (see [Done](#done)); everything else is here, roughly most
important first. Each entry says where the problem is, why it matters and what a fix could look like.
Line numbers are as of that branch.

Before changing words the player sees, read [writing.md](writing.md) (see `CLAUDE.md`). Before
changing how the game plays, read the design documents it lists.

## Robustness

### Lumen can stay "waiting" forever

`crates/tutor/src/provider.rs:86` limits the wait for the response headers to 120 s, but there is no
limit on the streamed body and no way to cancel. If the service stalls mid-reply,
`Conversation::is_waiting` stays true and the Ask button for that puzzle stays disabled until the game
restarts.

- Add a body timeout (`ureq`'s `timeout_recv_body`) or an overall one, plus a cancel button in
  `ui::tutor_body` that drops the pending request (the request thread already stops when its sender
  fails).
- `read_event_stream` (`provider.rs:131`) returns `Ok` after `[DONE]` even when no text came, so the
  player sees nothing at all. Report an empty reply as a `Turn::Problem`, and say so when
  `finish_reason` is `"length"` (the reply was cut off).

### Saves can be lost

- `Game::load_or_new` (`game/src/state.rs:50`): if the save can't be read (an old version, a file
  damaged by a crash, a permissions error), a new game starts and the next `persist` overwrites the
  old file. Copy the unreadable file aside (`slot1.ron.bak`) before the first write, and say so in
  `save_problem`.
- `Game::persist` (`state.rs:158`) writes the file in place. Write to a temporary file in the same
  directory and rename it over the save, so a crash mid-write leaves the old save intact.

### The opening replays

`companion.rs:539` treats a save as fresh when no test has been run. A player who has measured, noted
readings or watched experiments, but not committed, hears the whole opening again on every launch.
Base it on the save being untouched (empty notebook, no pages, no experiments, no tests), or keep an
"opening heard" flag in the save.

## The tutor and the leak filter

### The player's own committed values are redacted

`TutorContext::build` (`crates/tutor/src/context.rs:220`) treats notebook values, the plates and the
experiment's numbers as already known, but not the values the player committed in tests. When Lumen
repeats a near miss ("your 10.4 m/s fell short"), the number is redacted. Add every committed value
of this puzzle's attempts (in SI and in its display unit) to `known_numbers`.

### Dashes in the prompt

The prompt's own labels use "—" (`crates/tutor/src/prompt.rs:168, 210, 232, 236`) while the policy
tells the model not to use dashes, which invites it to copy them. Use colons or brackets. The
end-to-end tests match some of these strings (`crates/tutor/tests/end_to_end.rs`), so update them
together.

## Gameplay and interface

- **Wrong station hint.** `game/src/ui.rs:240` says "Press E to use the station" whenever the
  puzzle is unlocked, but `stations::proximity` also needs the console to be awake (after the wake
  scene). Use the same condition in both places. "This station is still asleep. Solve Catch the
  crawler first." also reads awkwardly; name the place instead ("Wake the haul track first").
- **Notebook clutter.** `sim::note_crawler` (`game/src/sim.rs:1110`) adds a new, identical stopwatch
  reading (t1, t2, t3...) on every run of the haul track's experiment. Note it once, or only when the
  reading is new.
- **Calculator messages** (`crates/physics_core/src/expr.rs`):
  - `sin 30` reports "'sin' isn't a notebook entry, a unit or a function". It is a function; it
    needs brackets. Say so when a function name is not followed by `(`.
  - "1 000 m" is read as 1 × 0 m = 0 (a number followed by a number multiplies). Refuse two plain
    numbers in a row, or warn.
  - Error positions are 0-based character indices. Show them 1-based, or point at the character.
- **Number formatting.** `units::sig` (`units.rs:224`) promises four significant figures but prints
  `123456` for 123456, and rounding can add a digit (9.9996 becomes "10.000"). Entering a bare 58
  for the ridge's angle gives "58.00 radians isn't a launch angle. Did you mean 58.00 deg?"
  (`puzzle.rs:731`); print the player's number as typed.
- **Minor input.** Only the left Shift runs (`player.rs:228`). Space also starts the game from the
  menu (`intro.rs:439`), which isn't documented, and a held Space then makes the avatar jump.

## Words that break docs/writing.md

- An em dash in the station's "All tests" list (`ui.rs:418`).
- The gravity brief speaks as Lumen ("the only value I carry is the one from home") in
  `assets/curriculum/tier1_ch1.ron`; briefs are plain instructions.
- The ridge brief says to leave room "because the tiller isn't a point", but the simulator treats it
  as one (`puzzle.rs`, `fly`). Either drop the reason or give the tiller a size in the judgement.
- "Does what it's told and works nothing out" appears in the intro, the gravity brief and its `why`
  line; writing.md asks for a different phrasing each time.
- Typos: "the cart's cart's block" (`game/src/carry.rs:47`); "With 1 blocks" in the channel's worked
  solution (`puzzle.rs:1120`).
- writing.md says a `task` starts with a verb and ends with a full stop. That is easy to check in
  `Chapter::style_problems` and isn't yet.

## Design question

At the drop tower, a failed test prints the mast's real fall time to three figures, from which
g = 2h/t² follows directly. One throwaway test therefore solves the puzzle without the light gate.
The other stations have a test that plays a guessing player against them (`tests_by_guessing` in
`puzzle.rs`); this one doesn't. Decide whether that is acceptable (it is still a measurement) or
whether the result should report less, then add the guesser test either way.

## Duplication and cleanup

- "The puzzle of this kind" is looked up in four places: `rig::puzzle_of` (`rig.rs:60`),
  `sim::solved`, and twice in `carry.rs`. Make it a `Game` method.
- `ui::valid_name` repeats the calculator's list of functions. Export it from `expr.rs`
  (`is_function`).
- Notebook sources are compared as plain strings: `"plate"` (`ui.rs:431`, `prefill.rs:95`),
  `"light gate"` (`companion.rs:741`, `ui.rs:513`, `prefill.rs:144`), and the `CRAWLER_TIME` label.
  Use constants or an enum.
- Repeated formulas:
  - the ridge's angle window, in `Ch1Layout::generate` (`puzzle.rs:152`) and `ridge_angle_window`
    (`puzzle.rs:342`);
  - the speed for an angle, `sqrt(g D / sin 2θ)`, in `puzzle.rs`, `context.rs` and the tests;
  - the stopwatch's rounding to 0.01 s, in `crawler_time` and `gate_time` (`puzzle.rs:296, 330`);
  - the bounding-box fold, in `stations::station_view` and `sim::handle_requests`.
- `Ch1Layout::channel_shot` rebuilds the channel's profile on every call, including inside loops over
  stacks and seeds. Build it once.
- The code calls the builders' blocks "crates" (`CrateMark`, `crate_pile`, `crate_yard`); the game's
  words say "block". Pick one.
- Clippy warns twice: a complex type in `devscript.rs:874` and eight arguments to `sim::relay`
  (`sim.rs:1145`).

## Tooling

- **Toolchain.** `Cargo.toml` asks for Rust 1.97.1. A machine on 1.97.0 refuses to build; use
  `cargo +1.97.1` or add a `rust-toolchain.toml` pinning it.
- **Formatting.** There is no `rustfmt.toml`, and `cargo fmt --check` reports 1175 differences (540
  with `max_width = 160`). Choose a configuration that matches the house style, run it once and check
  it.
- **CI.** There is none. A workflow running `cargo test` and `cargo clippy` for the workspace (the
  logic crates take seconds; the game takes a few minutes to build) and `cargo fmt --check` would
  catch regressions.

## Stale documentation

- `docs/game-design.md:223` and `crates/physics_core/src/content.rs:603` say measuring gravity is
  "the first puzzle". It is the third (`1.1.2`).
- `docs/architecture.md`: the `physics_core` table lists a `save` module, which is called
  `progress`. The game's table leaves out `island`, `scenery`, `props`, `state` and `prefill`.
- `README.md:82` says "Three scripted runs" and lists four; `way` and `prefill` are documented only in
  `architecture.md`.
- `docs/screenshots/island.png`, `result.png` and `ruler.png` aren't used by any document.
- The game design's core loop, step 5 ("the part is fabricated and appears on the ship"), predates the
  way waking.
- The rating plate's doc comment (`ui.rs:426`) says "its one speed"; it is a band.

## Done

Fixed on `claude/confident-ptolemy-hz598f`, with tests:

1. **Duplicate tiller and casket.** The casket now rests on the platform once it has crossed, and the
   tiller leaves the tilting launcher once it is in the skiff (`sim::shown`, `sim::rest_pose`). A test
   run again after a pass still flies from the launcher and doesn't deliver a second tiller.
2. **The world changed before a request was judged.** `sim::accept` judges a request first; a
   refused commit or experiment leaves the world as it was.
3. **Quitting during a test avoided its charge.** `Game::begin_test` uses the charge and writes the
   whole attempt to the save file at commit; `Game::finish_test` counts it in the game once it has
   played out.
4. **Lumen pointed at old positions.** `Landmarks::follow` keeps the moving pins (the ledge
   launcher's release point, the light gate) and the rail cart in step.
5. **Channel answers leaked in common phrasings.** Units now match as whole words, a count may have
   a couple of words before "blocks", and each stack's exact speed is guarded without its unit.
6. **Small whole numbers were redacted.** For an answer that is measured rather than counted, a
   whole number counts only with the answer's unit after it.
