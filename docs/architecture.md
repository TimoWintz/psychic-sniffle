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
| `voxel` | fixed-size block grid, island generator driven by the chapter layout, face-culling mesher, ray cast |
| `player` | avatar, kinematic movement against the block grid, orbit camera |
| `stations` | station entities, proximity and interaction, the active puzzle |
| `sim` | experiment and test playback, hand-off to the physics engine |
| `ruler` | two-point measurement tool |
| `ui` | panels: station (brief, instruments, blueprint, result), notebook and calculator, handbook, chat |
| `shipyard` | ship parts that appear as chapters complete |
| `tutor_bridge` | builds the snapshot, starts a request, polls for streamed text |
| `devscript` | scripted run for automated screenshots (development only) |

The island is generated from the same seeded layout that the puzzles read, so a distance measured in
the world and the number the simulator uses are the same number.

## tutor

See `tutor.md`. The network request runs on its own thread and reports through a channel that the game
polls once per frame; the game never blocks on the network.

## Interface

Panels use `bevy_egui` (text entry, scrolling, simple plots drawn directly).

## Testing

- `cargo test -p physics_core` — units, calculator, simulators against closed-form results, every puzzle
  solvable across 1,000 seeds, a 10 % wrong answer fails.
- `cargo test -p tutor` — snapshot, prompt, leak filter, adapter against a local stand-in server.
- `GAME_SCRIPT=<name> cargo run -p game` — scripted run that plays a puzzle and writes screenshots.
