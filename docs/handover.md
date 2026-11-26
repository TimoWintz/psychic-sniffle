# Handover: issues from the November 2026 review

A review of the whole repository found a list of issues. All of them are now fixed (see [Done](#done)).

Before changing words the player sees, read [writing.md](writing.md) (see `CLAUDE.md`). Before
changing how the game plays, read the design documents it lists.

## Done

### Third round

- **Drop tower shortcut closed.** A failed test at the drop tower now reports only whether the real
  mass landed sooner or later than predicted (never the mast's observed fall time), and omits the
  comparison row on a miss. `guessing_at_the_drop_tower_takes_more_tests_than_a_station_has_charges`
  checks that guessing now takes well over a station's three charges on average.

Fixed on `claude/confident-ptolemy-hz598f`, with tests where a program can check it.

### First round

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

### Second round

Robustness:

- **Lumen could stay "waiting" forever.** The request now also stops after 60 s of silence or 300 s
  of reply (`provider.rs`), and the chat has a Stop button while a reply is on its way
  (`Conversation::cancel`, which keeps what has arrived). A stalled request's thread lingers until
  its timeout, but nothing waits for it. An empty reply, even one ended with `[DONE]`, is a problem,
  and a reply cut off at `max_tokens` keeps its text and says so (`TutorError::CutOff`).
- **Saves could be lost.** A save that exists but can't be read is renamed aside (`slot1.ron.bak`,
  then `.bak2`...) before anything is written, and the dock says where it went. If even that fails,
  the new game isn't saved at all (`Game::may_write`). Saving writes a new file and renames it over
  the old one (`state::write_replacing`).
- **The opening replayed.** Lumen's opening now plays only while the save is untouched
  (`Save::untouched`: nothing measured, noted, watched, placed, read or tested).

The tutor:

- **The player's own committed values were redacted.** Every value committed in the puzzle's tests,
  in SI and in its display unit, is a known number.
- **Dashes in the prompt.** The snapshot's labels use colons and brackets now, and the end-to-end
  tests match.

Gameplay and interface:

- **Station hint.** The hint and E share `stations::ready`. A station that is unlocked but still
  waking says so, and a sleeping one names the machine to finish first ("Finish the job at the haul
  track first.").
- **Notebook clutter.** The crawler's stopwatch time, and a light gate pair, are noted once
  (`Notebook::has_reading`).
- **Calculator.** `sin 30` says that sin is a function and needs brackets, two numbers in a row
  ("1 000") are refused, and error positions count characters from 1.
- **Number formatting.** `units::sig` rounds whole numbers to its significant figures (123500) and
  measures again after rounding (9.9996 is "10.00"). The ridge's bare-angle refusal shows the number
  as typed ("58 radians... Did you mean 58 deg?").
- **Input.** Either Shift runs. A Space press held over from the menu doesn't jump. The README says
  Space starts the game too.

Words:

- The em dash in the station's "All tests" list is a colon.
- The gravity brief is a plain instruction, and no longer repeats "do what they're told".
- The ridge brief's reason for leaving room is now true: no measurement is perfectly exact.
- Typos: "the cart's cart's block" (two places), "With 1 blocks" in the channel's worked solution.
- `Chapter::style_problems` checks each `task`: one sentence, a capital letter, a full stop, and not
  starting with a word an instruction can't start with ("The", "We'll"...).

Duplication and cleanup:

- `Game::puzzle_of` and `Game::kind_solved` replace the four lookups by kind.
- `ui::valid_name` uses `expr::is_function`.
- Notebook sources are `notebook::source` constants (the save still holds the same words).
- Shared formulas: `ballistics::level_speed` (v = sqrt(g D / sin 2θ)), `ridge_min_angle` and
  `ridge_shape`, `stopwatch` (rounding to 0.01 s), `stations::bounds`. The layout's numbers are
  unchanged, so old saves keep them.
- `Ch1Layout::channel_shot_over` takes a profile built once; the fairness check and the layout test
  use it.
- The code says "block" for the builders' blocks (`BlockMark`, `block_pile`, `block_yard`,
  `HeldBlock`). The tutor's filter still knows "crate" as a word a model may use.
- Both clippy warnings are gone.

Tooling:

- `rust-toolchain.toml` pins Rust 1.97.1, with clippy and rustfmt.
- `rustfmt.toml` (`max_width = 160`, `use_small_heuristics = "Max"`,
  `short_array_element_width_threshold = 20`) is the closest match to the house style; the whole
  workspace was formatted with it once, in a commit of its own.
- CI (`.github/workflows/ci.yml`) runs formatting, clippy with warnings denied, and the tests.

Stale documentation:

- Gravity is the third puzzle, in `game-design.md` and the comment in `content.rs`.
- `architecture.md` names `progress` and lists `state`, `island`, `scenery`, `props` and `prefill`.
- The README lists all five scripted runs, `way` included.
- The unused screenshots (`island.png`, `result.png`, `ruler.png`) are deleted.
- The core loop's result step describes the way waking.
- The rating plate's comment says it is a band.
