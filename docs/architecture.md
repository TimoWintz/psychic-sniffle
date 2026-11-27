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
| `beam` | the beam, chapter 1.1's side-machine: its seeded loads, what a release does with the blocks on its seats, and the tests that the last load can't be fiddled into |
| `materials` | the materials table (density, specific heat, conductivity, resistivity, colour) |
| `puzzle` | chapter and puzzle definitions, seeded instance generation, evaluation of committed inputs, worked solutions |
| `demo` | each station's experiment, decided in closed form like a test: strobe times, the dropped and fired blocks, the two-angle shots |
| `content` | chapter words as data: briefs, concept pages and where each is found, each page's Earth scene and quiz, experiment questions, the side-machines off the road; the light the chapter is played in (`Light`, a preset the game draws) |
| `notebook` | named readings; the calculator's variables |
| `progress` | the save: seed, attempts, completed puzzles, notebook, pages found, demonstrations watched, what is placed, quiz questions answered right, each side-machine's blocks, loads held and wake |

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

A test counts from the moment it is committed: its charge is used then, and the save file holds the
whole attempt (`Game::begin_test`), so quitting while it plays cannot take it back. The game itself
shows the verdict, and counts the puzzle solved, only once the world has played it out
(`Game::finish_test`).

## game

| Module | Contents |
|---|---|
| `intro` | title menu (with 3-slot save management and quit) and the intro behind it: the placeholder sets for levels III and II, the ship, smoke and dust, the hand-over to the player's view |
| `cinematic` | the intro's timeline as plain functions: camera, ship parts, fades, captions and events at any moment of the loop (tested without the engine) |
| `state` | the shared game state: the chapter, the seeded layout, the save and its numbered slots (read-only inspection, moved aside if it can't be read, written whole by renaming a new file over it), the test still playing out, and what the interface shows |
| `session` | world lifecycle: builds save-dependent entities on `BuildWorld` at startup and cleanly despawns and rebuilds them when switching save slots, starting a new game, or returning from the pause menu |
| `island` | the island built from the chapter layout in whole blocks: the coast, the puzzle sites kept flat, the route along the ancient way from the wreck to the skiff, the places (`Sites`) everything else reads |
| `graphics` | the quality setting (`[graphics] quality` in `settings.toml`, or `GAME_QUALITY`) and the camera's look: high dynamic range, TonyMcMapface tonemapping, the colour grade, bloom, anti-aliasing (MSAA on low, SMAA on high), ambient occlusion and finer shadows on high; how strongly each glowing thing is drawn (`glow`), so the order of what glows is set in one place; the blocks' material (`BlockSurface`, its shader `blocks.wesl`), which draws a fine pattern for each material inside every face |
| `water` | the sea: one plane drawn by its own shader (`water.wesl`) from a map baked from the block grid, one texel per column, of the water's depth and how near the land is (shallows and depths, foam at shores and cliffs, surf over a reef); the map is worked out again around blocks at the water's level when they change |
| `particles` | small things in motion, each a tiny cube: drops where something falls into the water, dust where a block is set down or stonework moves, spray at cliffs, motes from the awake quartz, and a wake's flourishes (the light's bright head and its trail, the burst as a machine wakes, the way's quartz brightening once); others ask for a burst with `particles::burst`; nothing drifting is made while a test, an experiment or the ruler is going |
| `aloft` | the sky's motion: clouds drifting high on the wind (no shadows), flocks circling far out over the sea (a test keeps them off the island) |
| `scenery` | everything looked at and never touched: the island's meshes, the sea bed, the sky, haze, distant islands and small plants (which lean in the wind: `Sway`, its vertex shader `sway.wesl`); the sun, the sky's colours and the haze for each of the chapter's lights (`Sky::of`) |
| `props` | the shapes of what tests move and of the wreck's loose debris, each inside the box the simulation reasons with |
| `voxel` | fixed-size block grid, island generator driven by the chapter layout, face-culling mesher (each vertex carries its block's material in its first UVs, for the blocks' shader), ray cast; glowing blocks that are dark for now (the way's sleeping quartz) |
| `player` | avatar, kinematic movement against the block grid, orbit camera |
| `avatar` | the avatar's jointed body and its animation: idle, walk and run, jump and land, working a console, watching a test, reacting to the result, looking where Lumen looks |
| `stations` | the builders' consoles and lamps (their quartz lit as each machine wakes), proximity and interaction, the active puzzle, and the camera's view of its puzzle while a station is open |
| `sim` | experiment and test playback, hand-off to the physics engine (whose view of the island is built again when the way's blocks come or go), handbook pages written by what is seen |
| `rig` | launchers that show the blueprint: the tilting arm and the speed gauges |
| `ruler` | two-point measurement tool |
| `carry` | carrying: pick up, the ghost of where it goes, put down; the slots stations read (the rail cart's block, the light gate's notch on the drop tower, the ledge launcher's jack frame, which raises it), and the beam's seats |
| `beam` | the beam off the road: its console and panel, the beam and its blocks as entities that turn together on a release, a stud lit for each load held, and its wake (its quartz and lamp, then the path to the road, lit while the player watches, with no letterbox) |
| `ui` | panels: station (brief, instruments, blueprint, result), notebook and calculator, the handbook's frame, chat |
| `handbook` | the handbook's pages: words with formulas boxed, each page's looping Earth picture (motion in closed form by `tracks`, tested without the engine), the optional quiz |
| `vessel` | the builders' skiff on its slipway, which takes its helm once the north gate wakes, and the sight the crawler carries (where it is, `waking` says) |
| `waking` | the ancient way's machines asleep and awake: everything follows from the stage (how many puzzles are solved). Each machine's quartz has a wake set (lit when the light reaches it) and a done set (lit when its own job is done); its stonework rises or sinks step by step (layer by layer for the ramp, the stone gate's leaf and the seal in the pass through the spine; the bridge over the channel rises pylons first, then its deck out from both banks, with its cables, which are meshes); each machine's console and lamps light as it wakes (`stations` draws them). The wake that plays after a pass (a pure timeline: the job's quartz lighting, the crane lifting the sight off after the crawler's pass, the bridge rising after the casket's, the light running up the road, the machine waking and its console lighting, the camera gliding home), its camera and its letterbox; the quartz on the machines' moving parts |
| `tutor_bridge` | builds the snapshot, starts a request, polls for streamed text |
| `companion` | Lumen's on-screen eye: the landmarks it can look at, the director that picks where it looks and what it says, drawing the eye, beam and speech |
| `devscript` | scripted run for automated screenshots (development only); `way` photographs each machine asleep, waking and awake; `look` photographs fixed views and times their frames, the benchmark for the look |
| `prefill` | `GAME_SCRIPT=prefill` (development only): notes the tour's ruler readings and keeps each blueprint filled with the tour's formula, while the player plays by hand |

The island is generated from the same seeded layout that the puzzles read, so a distance measured in
the world and the number the simulator uses are the same number.

The island is built awake, with the way's quartz lit and the ramp onto the outcrop standing, and
every island test checks that world. `waking::settle` puts it to the save's stage in `main`, before
anything is meshed or collides; a test checks that sleeping and waking stage by stage gives back the
island as built, that the ledge is out of reach until the ramp rises, that the north island is
out of reach until the bridge stands (the channel runs from coast to coast, and the avatar can't cross
water too deep to wade, `island::too_deep`: it falls in and comes back where it stepped off), that the skiff is out of reach until the seal sinks after the last
puzzle (the spine runs from coast to coast, a reef under the water at both ends, and nothing climbs
it), and that neither launcher's line is touched by the way at any stage.
When the way's blocks change for good (as a wake ends, or on a new game), `sim` builds the physics
engine's static collider again, so tumbling debris meets the island the avatar walks on.

The block grid is not quite fixed: movable blocks are blocks of the world too, so they can be stood on and
measured. When one is picked up or put down, `carry` lists the changed cells and `scenery` meshes
those chunks again. A puzzle input can be **placed** rather than typed (`InputSource::Placed`): the
blueprint then reads it from the world (the blocks in the jack frame), and the save keeps it.

## Building placed puzzles

What a station reads from the world is a `carry::Slot`: `Cart` (one block), `Tower` (the light gate
at a notch), `Frame` (a stack of blocks). Each holds one number, which is the save's `placed` value
for its puzzle. Adding a placement is adding a slot:

- `carry::target_for` says what `F` would do from where the avatar stands and faces (and where the
  pointer marks the tower); it is pure and tested without the engine. `carry::apply` does it, and is
  also what the scripted tour calls. `carry::set_slot` makes the world show what a slot holds
  (blocks, the launcher's height, a pin) and lists the changed cells for re-meshing.
- `physics_core` says what a placement means: `Instance::built` puts it in words (station panel,
  tutor) and `Instance::unready` refuses a test until something is in place. `sim::judge_commit`
  runs that check before anything typed is read, so such a refusal writes no handbook page.
- Things that move with what is placed follow `Sites` changing: `ruler::follow_pins` (pins by index,
  such as `Sites::channel_release_pin` and `Sites::gate_pin`; a pin at `Pin::AWAY` is not in the world),
  `sim::follow_launcher`, `rig::ride_the_stack`. Lumen's director (`companion::choose_gaze`) is pure
  and tested; its `Facts` say what is carried and whether the puzzle's slot is empty.

Rules learned the hard way:

1. **Measure how fast a guess finds the answer.** Every placed answer gets a guesser test in
   `puzzle.rs` (`tests_by_guessing`); see "What makes guessing a bad strategy" in the game design.
2. **Old saves keep their numbers.** New layout numbers are drawn after the ones already drawn at the
   end of `Ch1Layout::generate`; draws no longer used stay in place, discarded. Extend
   `older_saves_keep_every_number_but_the_channels` with any number you touch.
3. **Leave room for the player's own measurement.** Later puzzles use the player's `g`, good to about
   0.5 %. Nothing that decides pass or fail may sit closer than that to an edge (`BAND_MARGIN`, 1.5 %,
   is the ledge launcher's).
4. **Stay on the flat ground the island keeps.** Shots and placements stay inside the `Plan` zones in
   `island.rs`; island tests compare the terrain along each launch line with the simulator's profile.
5. **Hide placed answers from Lumen without over-redacting.** A small whole number is too common to
   redact everywhere: scope each secret by the unit words that follow it (`tutor/src/context.rs`, the
   ridge and the channel), and put numbers in plain view (plates, the gate's notch) in `known_numbers`.
6. **Stations freeze the avatar.** Carrying happens with the panel closed, and a commit reads what is
   placed at that moment. Walking away from a console closes its panel.
7. **Movable blocks are blocks, not entities.** One is `Material::Limestone` in the voxel world plus
   a `BlockMark`, put in after `sim::setup` so the physics engine's static collider never contains it.
   Something that is not a whole block (the light gate, the block riding the cart) is an entity.

## Building side-machines

A side-machine (see the game design) is not a puzzle: it has no index among the stations, no stage of
the way and no charges, so nothing that counts puzzles sees it.

- Its words are a `SideDef` in the chapter's `sides`, and its page is found by `Found::Side(id)`.
  `Chapter::problems` checks that each writes a page, that no puzzle needs that page, and that its
  spoken lines hold no digits; remarks may name it in `Before` and `After`.
- The save keeps it in `Save::sides`: what is built on it, how many of its cases have held, whether
  it has woken. `Save::is_found` counts a woken one's page as found.
- Its logic lives in its own `physics_core` module (`beam`), with a test that the last case can't
  be reached from the one before by one move, and that few of everything the pile allows solve it.
- In the game it is one module (`beam`): its place in `Sites` (built by `island`, with a zone of
  flat ground and a path of quartz to the road, which `beam::settle` darkens in `main` until it wakes),
  its console (`Ui::beam_open` and `Ui::near_beam`; `Ui::at_console` covers stations and it alike,
  for what freezes the avatar or quiets Lumen), and its own pieces of `carry` (a `Yard`, and targets
  of its own instead of a `Slot`). Lumen's director watches its releases and its wake; the tutor's
  snapshot names it, and whether it is awake, and nothing more.

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

CI (`.github/workflows/ci.yml`) runs `cargo fmt --check`, `cargo clippy` with warnings denied, and
`cargo test` for the whole workspace, on the toolchain `rust-toolchain.toml` pins.

- `cargo test -p physics_core` — units, calculator, simulators against closed-form results, every puzzle
  solvable across 1,000 seeds, a 10 % wrong answer fails.
- `cargo test -p tutor` — snapshot, prompt, leak filter, adapter against a local stand-in server.
- `cargo test -p game` — the island against the simulators' assumptions, the intro's timeline (pieces
  land at their wrecks, nothing flies through the ground, no visible jumps, the hand-over ends on the
  player's view).
- `GAME_SCRIPT=<name> cargo run -p game` — scripted run (`tour`, `walk`, `intro`, `pose`, `way`, `beam`) that drives
  the game and writes screenshots. The tour loads the rail cart, times the crawler and catches it,
  times drops with the light gate at two notches, and carries the ledge's blocks into the jack frame. Headless, it runs under
  `xvfb-run -a -s "-screen 0 1920x1080x24"` (with `libxkbcommon-x11-0` and `mesa-vulkan-drivers`
  installed) and takes about ten minutes.
- `GAME_SCRIPT=prefill GAME_SAVE=/tmp/fp-prefill.ron cargo run -p game` — not a script: the player plays by hand while the
  tour's ruler readings are noted and its formulas are kept in the blueprints (`game/src/prefill.rs`).
  Without `GAME_SAVE` it writes into the real save, `saves/slot1.ron`.
