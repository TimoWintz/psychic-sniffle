# Architecture

Rust workspace, three crates. The two logic crates have no engine dependency: they test in seconds, and
the tutor's isolation from the game is a property of the dependency graph.

```
crates/physics_core   units, calculator, exact simulators, puzzle model      (no Bevy)
crates/tutor          snapshot, prompt, provider adapters, leak filter       (no Bevy)
game                  Bevy application: world, avatar, stations, interface
assets/curriculum     chapter content as data (.ron)
docs                  design documents
```

`game` depends on both logic crates. `tutor` depends on `physics_core` for shared data types only.
Nothing depends on `game`.

## physics_core

| Module | Contents |
|---|---|
| `units` | `Dim` (exponents over the SI base units), `Quantity` (SI value + `Dim`), unit table, formatting |
| `expr` | calculator: parser and evaluator for expressions with units and named variables; reports dimension errors |
| `ballistics` | closed-form projectile motion: position at a time, time and place of landing, height at a distance |
| `materials` | the materials table (density, specific heat, conductivity, resistivity, colour) |
| `puzzle` | chapter and puzzle definitions, seeded instance generation, evaluation of committed inputs, worked solutions |
| `demo` | each station's experiment, decided in closed form like a test: strobe times, the dropped and fired crates, the two-angle shots |
| `content` | chapter words as data: briefs, concept pages and where each is found, experiment questions |
| `notebook` | named readings; the calculator's variables |
| `save` | progress record (seed, attempts, completed puzzles, notebook) |

A puzzle is a **kind** (Rust code: how to generate numbers from a seed, how to judge an attempt, how to
write the worked solution) plus **content** (data: title, brief, concept cards, number ranges). Adding a
puzzle of an existing kind is a data change; a new kind is a code change with tests.

## Two physics layers

- **Exact simulators** (`physics_core`) decide every outcome. They are closed-form or fixed-step
  integrators with tests against analytic results. The verdict never depends on frame rate.
- **The physics engine** (`avian3d`) is for illustration: debris, tumbling, bouncing, collapse.

Hand-off rule: the exact simulator moves an object until the event that is judged (a landing, a strike).
At that instant the verdict is recorded and the object becomes an engine-driven body with the velocity
it had. What the engine does afterwards cannot change the result.

## game

| Module | Contents |
|---|---|
| `intro` | title menu and the intro behind it: the placeholder sets for levels III and II, the ship, smoke and dust, the hand-over to the player's view |
| `cinematic` | the intro's timeline as plain functions: camera, ship parts, fades, captions and events at any moment of the loop (tested without the engine) |
| `voxel` | fixed-size block grid, island generator driven by the chapter layout, face-culling mesher, ray cast |
| `player` | avatar, kinematic movement against the block grid, orbit camera |
| `avatar` | the avatar's jointed body and its animation: idle, walk and run, jump and land, working a console, watching a test, reacting to the result, looking where Lumen looks |
| `stations` | station entities, proximity and interaction, the active puzzle |
| `sim` | experiment and test playback, hand-off to the physics engine, handbook pages written by what is seen |
| `rig` | launchers that show the blueprint: the tilting arm and the speed gauges |
| `ruler` | two-point measurement tool |
| `carry` | carrying: pick up, the ghost of where it goes, put down; the slots stations read (the rail cart's cargo, the light gate's notch on the drop tower, the canyon's jack frame, which raises the launcher) |
| `ui` | panels: station (brief, instruments, blueprint, result), notebook and calculator, handbook, chat |
| `shipyard` | ship parts that appear as chapters complete |
| `tutor_bridge` | builds the snapshot, starts a request, polls for streamed text |
| `companion` | Lumen's on-screen eye: the landmarks it can look at, the director that picks where it looks and what it says, drawing the eye, beam and speech |
| `devscript` | scripted run for automated screenshots (development only) |

The island is generated from the same seeded layout that the puzzles read, so a distance measured in
the world and the number the simulator uses are the same number.

The block grid is not quite fixed: movable crates are blocks too, so they can be stood on and
measured. When one is picked up or put down, `carry` lists the changed cells and `scenery` meshes
those chunks again. A puzzle input can be **placed** rather than typed (`InputSource::Placed`): the
blueprint then reads it from the world (the crates in the jack frame), and the save keeps it.

## Building placed puzzles

What a station reads from the world is a `carry::Slot`: `Cart` (one crate), `Tower` (the light gate
at a notch), `Frame` (a stack of crates). Each holds one number, which is the save's `placed` value
for its puzzle. Adding a placement is adding a slot:

- `carry::target_for` says what `F` would do from where the avatar stands and faces (and where the
  pointer marks the tower); it is pure and tested without the engine. `carry::apply` does it, and is
  also what the scripted tour calls. `carry::set_slot` makes the world show what a slot holds
  (blocks, the launcher's height, a pin) and lists the changed cells for re-meshing.
- `physics_core` says what a placement means: `Instance::built` puts it in words (station panel,
  tutor) and `Instance::unready` refuses a test until something is in place. `sim::judge_commit`
  runs that check before anything typed is read, so such a refusal writes no handbook page.
- Things that move with what is placed follow `Sites` changing: `ruler::follow_pins` (pins by index,
  such as `Sites::canyon_release_pin` and `Sites::gate_pin`; a pin at `Pin::AWAY` is not in the world),
  `sim::follow_launcher`, `rig::ride_the_stack`. Lumen's director (`companion::choose_gaze`) is pure
  and tested; its `Facts` say what is carried and whether the puzzle's slot is empty.

Rules learned the hard way:

1. **Measure how fast a guess finds the answer.** Every placed answer gets a guesser test in
   `puzzle.rs` (`tests_by_guessing`); see "What makes guessing a bad strategy" in the game design.
2. **Old saves keep their numbers.** New layout numbers are drawn after the ones already drawn at the
   end of `Ch1Layout::generate`; draws no longer used stay in place, discarded. Extend
   `older_saves_keep_every_number_but_the_canyons` with any number you touch.
3. **Leave room for the player's own measurement.** Later puzzles use the player's `g`, good to about
   0.5 %. Nothing that decides pass or fail may sit closer than that to an edge (`BAND_MARGIN`, 1.5 %,
   is the canyon's).
4. **Stay on the flat ground the island keeps.** Shots and placements stay inside the `Plan` zones in
   `island.rs`; island tests compare the terrain along each launch line with the simulator's profile.
5. **Hide placed answers from Lumen without over-redacting.** A small whole number is too common to
   redact everywhere: scope each secret by the unit words that follow it (`tutor/src/context.rs`, the
   ridge and the canyon), and put numbers in plain view (plates, the gate's notch) in `known_numbers`.
6. **Stations freeze the avatar.** Carrying happens with the panel closed, and a commit reads what is
   placed at that moment. Walking away from a console closes its panel.
7. **Crates are blocks, not entities.** A crate is `Material::Timber` in the voxel world plus a
   `CrateMark`, put in after `sim::setup` so the physics engine's static collider never contains it.
   Something that is not a whole block (the light gate, a crate riding the cart) is an entity.

The game starts in `Mode::Menu` (title menu, intro playing). The avatar, camera, stations, panels and
Lumen only run in `Mode::Playing`, which Start switches to once the camera has glided to the player's
view. Everything the intro made is despawned then, except the mainland, which stays as the far shore of
the strait.

## tutor

See `tutor.md`. The network request runs on its own thread and reports through a channel that the game
polls once per frame; the game never blocks on the network.

## Interface

Panels use `bevy_egui` (text entry, scrolling, simple plots drawn directly).

## Testing

- `cargo test -p physics_core` — units, calculator, simulators against closed-form results, every puzzle
  solvable across 1,000 seeds, a 10 % wrong answer fails.
- `cargo test -p tutor` — snapshot, prompt, leak filter, adapter against a local stand-in server.
- `cargo test -p game` — the island against the simulators' assumptions, the intro's timeline (pieces
  land at their wrecks, nothing flies through the ground, no visible jumps, the hand-over ends on the
  player's view).
- `GAME_SCRIPT=<name> cargo run -p game` — scripted run (`tour`, `walk`, `intro`, `pose`) that drives
  the game and writes screenshots. The tour loads the rail cart, times drops with the light gate at
  two notches, and carries the canyon's crates into the jack frame. Headless, it runs under
  `xvfb-run -a -s "-screen 0 1920x1080x24"` (with `libxkbcommon-x11-0` and `mesa-vulkan-drivers`
  installed) and takes about ten minutes.
