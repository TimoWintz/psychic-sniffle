# Handover: carry and place

For whoever builds the next "doing it with your hands" puzzles. The verb and one puzzle are built;
three redesigns are specified here but not built. Read `game-design.md` ("Doing it with your hands")
and `curriculum.md` (slice specification) first. This page is about how to build the rest.

## Where things stand

**Built** on `claude/puzzle-interactivity-brainstorm-kzqa5j` (commits `fbd07f3`, `83c8323`,
`cc18b8a`):

- **The verb.** `F` picks up a salvage crate in front of the avatar and puts it down at the ghost
  block, on the ledge north of the canyon launcher's rail. Crates are voxels: they can be stood on
  and measured.
- **The canyon, as "build, then compute".**
  - The ledge launcher's dial turns only within a band (`canyon_speed_min..=canyon_speed_max`).
  - From the bare ledge, the speed needed is above the band.
  - The player works out which release heights the band allows and builds one with crates in the
    jack frame at the rim.
  - Then they type the speed for the height built. The target is a one-block strip.

**Planned, not built:**

| | Redesign | Pattern |
|---|---|---|
| A | Supply rail (tutorial): load the cargo crate onto the cart | teaches the verb |
| B | Local gravity: a light gate clamped at a chosen height on the drop tower | place to measure |
| C | Chapter 1.3's sled: place the stop where friction brings it to rest | place the catcher |

## The machinery to reuse

| What | Where | Notes |
|---|---|---|
| What `F` would do | `carry::target_for(world, sites, carry, feet, forward) -> Option<Target>` | Pure, tested without the engine. `Target` is `Take`, `TakeFromFrame`, `Put`, `PutInFrame`, `Blocked(why)`. |
| Doing it | `carry::apply(...)`, `carry::set_stack(...)` | `apply` is also what the scripted tour calls. `set_stack` fills the frame column, moves `Sites::canyon_launch.origin` and the release pin, and lists the changed cells. |
| State | `carry::Carry` (`loose`, `stack`, `holding`, `target`) | Loose crates are not saved: they return to the pile at the next start. |
| Re-meshing | `carry::DirtyCells` → `scenery::rebuild_chunks` | Push every changed cell; the chunk and its four neighbours are meshed again. |
| Handles and colliders | `CrateMark` entities in `carry.rs` (`lay_out`, `place_marks`) | One per crate, with a static collider. Crates are put into the world *after* `sim::setup`, so the physics engine's static voxel collider never contains them. |
| Where crates may go | `Sites::crate_yard`, `Sites::crate_pile`, `Sites::canyon_well` (`island.rs`) | The yard keeps crates off launch lines and away from the station. |
| A placed input | `physics_core::puzzle::InputSource::Placed`; `Save::placed` (puzzle id → count); `Game::placed(puzzle)` | `sim::read_blueprint` reads placed inputs from the save. The station panel shows them read-only beside the typed ones. |
| Things that move with what is built | `ruler::follow_pins`, `sim::follow_launcher`, `rig::ride_the_stack` | Each runs when `Sites` changes. Pins are found by index (`Sites::canyon_release_pin`). |
| A value to note | `ui::rating_plate`, `ui::note_plate` | Writes `vmin` and `vmax` with source `"plate"`. |
| Lumen | `companion::Facts` (`carrying`, `frame_empty`); landmarks `SALVAGE_CRATES`, `JACK_FRAME`; the nudge when a station opens | `choose_gaze` is pure and tested. |
| Hints bar | `carry::hint` | Shown when no station is open. |
| The avatar | `avatar::carrying` (pose), `AvatarState::w_carry` | Cheers and slumps are suppressed while holding something. |
| Scripted runs | `devscript` steps `CarryAt { feet, facing, expect }`, `NotePlate`, `Calculate`, `ExpectSolved` | `CarryAt` stands the avatar somewhere, checks what `F` would do there, and does it. |
| Guessing | `tests_by_guessing` and `guessing_takes_more_tests_than_a_station_has_charges` in `puzzle.rs` | See the first rule below. |

## Rules learned the hard way

1. **Measure how fast a guess finds the answer.**
   - Every miss says short or beyond, so any answer can be found by halving after each miss.
   - "Chance" is really the number of tests that halving takes: target size against everything
     achievable, in bits.
   - Write a guesser test for every placed answer. Baselines from the canyon, with the same guesser:
     - old typed speed: 3.9 tests on average, 31 % within three;
     - crates only (fixed speed): 2.8 and 75 %;
     - now: 7.6 and 32 %.
   - A placement on its own (a handful of stacks) is too small. Pair it with a calculated value or a
     small target.
2. **Old saves keep their numbers.**
   - New layout numbers are drawn after `rail_target` at the end of `Ch1Layout::generate`.
   - Draws you no longer use stay in their place, discarded.
   - Extend `older_saves_keep_every_number_but_the_canyons` with any numbers you touch.
3. **Leave room for the player's own measurement.**
   - Later puzzles use the player's `g`, which is good to about 0.5 %.
   - Nothing that decides pass or fail may sit closer than that to an edge. `BAND_MARGIN = 1.5 %` is
     the canyon's margin.
4. **Stay on the flat ground the island keeps.**
   - Shots and placements must stay inside the `Plan` zones in `island.rs`.
   - Island tests compare the terrain along each launch line with the simulator's profile.
   - `Ch1Layout::CANYON_RUN` bounds the canyon's furthest shot. Redraw layouts that exceed it.
5. **Hide placed answers from Lumen without over-redacting.**
   - In `tutor/src/context.rs`, a small whole number is too common to redact everywhere.
   - Scope each secret by the unit words that follow it ("crate(s)", "m", "m/s"). The ridge and the
     canyon show how.
   - Put values printed in plain view (plates) in `known_numbers`.
6. **Stations freeze the avatar.**
   - Carrying happens with the panel closed, and a commit reads what is built at that moment.
   - Walking away from a console closes its panel (`stations::proximity`).
7. **Crates are blocks, not entities.**
   - A crate is `Material::Timber` in the voxel world plus a `CrateMark`.
   - Something that is not a whole block (a gate on a notch) needs a second movable kind; see B.

## A. Supply rail: load the cart

**Goal.** Teach `F` with nothing at stake, before the canyon needs it.

**Flow.**
1. A cargo crate stands beside the rail start.
2. The player carries it onto the cart.
3. The cart then carries it on its run.

The typed distance and the bare-number lesson are unchanged.

**Rules.**
- Committing with an empty cart is refused for free: "load the cart first".
- That refusal must not count as the first commit that writes the *Measurements carry units* page.
  `sim::handle_requests` finds pages (`PageEvent::Commit`) before reading the blueprint, so check
  the cart before that.

**Changes.**
- `carry.rs`: generalise the jack frame into named slots, e.g. `Slot { cell, puzzle, kind }` with
  kinds `Stack` (the frame) and `Single` (the cart), so both share `target_for` and `apply`.
- `island.rs`: the crate's starting cell beside the rail. Check it with the rail test
  (`the_supply_rail_is_clear_and_measures_true`).
- `sim.rs`: `Running::Slide` moves the crate with the cart, and puts it back afterwards.
- `devscript`: the tour loads the cart first.
- `tier1_ch1.ron`: brief and investigate text.

**Tests.**
- An empty cart is refused and costs no charge.
- The units page is still written by the first real commit.
- The crate's cell is free and on the ground.

**Open.** Must the crate be unloaded afterwards, or does it stay on the cart?

## B. Local gravity: a light gate

**Goal.** The player designs their own measurement: a gate clamped at a chosen height on the drop
tower, with each release timed from the release to the gate. They collect pairs (`h1, t1`,
`h2, t2`, ...) and can see that doubling the drop does not double the time. The answer stays the
typed `g`.

**Flow.**
- The gate is carried like a crate and fitted at any whole-metre notch of the tower, from 1 m below
  the release down to the plate.
- The experiment button releases the mass.
- With a gate fitted, the stopwatch notes the height and the time to the gate. Without one, it notes
  the full drop as it does now (`add_numbered("t", ...)` in `sim.rs`).

**Physics.**
- No new layout draws: the notches are `1..=tower_h`.
- Readings are `fall_time(h, g)` rounded to 0.01 s, like `tower_fall_time`.
- Short drops are worth less. At 1 m the time is about 0.4–0.55 s, so the rounding alone is up to
  about 1.3 % of the time and 2.6 % of `g`, beyond the 2 % pass.
  - A reading gives `g` to 1 % only when `t ≥ 1 s`, i.e. `h ≥ g/2` (3.3–6.8 m).
  - Decide whether the brief says so, or whether players find out by fitting several heights. Then
    extend `the_players_own_measurement_of_g_is_good_enough` to the rule you choose.

**Changes.**
- `carry.rs`: a second movable kind that is not a block. It is an entity that snaps to notches on the
  tower's column and is held like a crate (reuse `HELD_AT` and the carrying pose).
  - Targets: `Fit(notch)` and `Unfit`, or the generic slots from A.
- `island.rs`: notch positions along the tower (the column is `TOWER` in `island.rs`). The gate needs
  a ruler pin that follows it, the same way `canyon_release_pin` does.
- `sim.rs`: `Running::Strobe` stops the clock at the gate's height. The strobe marks still show the
  whole fall.
- `Save::placed["1.1.1"]` keeps the notch (0 for none).
- `ui.rs`: the experiment section shows the gate's height and the last reading.
- `companion.rs`: Lumen points at the gate once the experiment has been watched and no gate reading
  exists yet.
- The tour fits the gate at two heights before committing `g`.

**Tests.**
- The readings match `fall_time` at every notch.
- `g` worked out from a good-enough notch passes, for every seed.
- The save keeps the notch.
- The strobe test (`the_strobe_marks_equal_moments_and_growing_gaps`) still holds.

**Open.**
- One gate or two? Two gates also give an average speed between them.
- A plot of `h` against `t²` belongs to the grapher (tier 2), not here.

## C. Chapter 1.3: place the catcher

Direction only; chapter 1.3 has no puzzles in code yet.

- A sled is pushed onto a level run (or a ramp) by a fixed launcher, and friction brings it to rest.
- The push is read off a plate. The player places the stop block where the sled will stop, in a row
  of slots along the run.
- It needs:
  - a friction simulator in `physics_core`, tested against closed form;
  - the slot row (the generic slots from A);
  - a guesser test.
- A row of slots alone is a handful of options, the same trap the first canyon fell into (rule 1). So
  make the slots fine and the target tight, or combine the placement with a calculated value (the
  friction coefficient measured first from the tilt at which the sled slips).

## Running and checking

- **Toolchain:** Rust 1.97.1 (`rustup toolchain install 1.97.1`).
- **Headless runs:** install `libxkbcommon-x11-0` and `mesa-vulkan-drivers`, then:

  ```bash
  GAME_SCRIPT=tour GAME_SEED=7 GAME_SAVE=/tmp/fp-tour.ron GAME_SHOTS=/tmp/shots xvfb-run -a -s "-screen 0 1920x1080x24" cargo run -p game
  ```

  The tour takes about ten minutes; `pose` and `walk` are quicker.
- **Known failures:** five avatar pose tests (`a_planted_gait_...`, `the_gait_mirrors_...`,
  `a_cheer_...`, `a_slump_...`, `the_air_pose_...`) fail on `main` too. Expect "38 passed; 5 failed"
  in the game crate until they are fixed separately; anything more is yours.
- **Done means**, per redesign:
  1. physics tests pass, including a guesser test wherever the answer is placed;
  2. the tutor's secrets cover the placed answer;
  3. the tour reaches `ExpectSolved` for the puzzle;
  4. `curriculum.md`, the patterns table in `game-design.md`, and the README's controls (if keys
     change) say what was built.
