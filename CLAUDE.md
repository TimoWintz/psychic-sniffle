# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

First Principles is a voxel physics game in Rust (Bevy 0.20, bevy_egui, avian3d). The player earns ship parts
by measuring the world and committing values that an exact simulator then judges. The repo holds the design
docs (`docs/`) and a playable slice, tier 1 chapter 1 (kinematics: a tutorial and three puzzles).

## Commands

The workspace sets `rust-version = "1.97.1"`. If the installed toolchain is older (for example 1.97.0), add
`--ignore-rust-version` to cargo build, run, test and clippy commands.

```bash
cargo run -p game                      # title menu with the intro behind it; Start/Enter to play
cargo test --workspace
cargo test -p physics_core             # fast, no engine
cargo test -p tutor                    # includes tests/end_to_end.rs (local stand-in server)
cargo test -p physics_core puzzle::    # filter by module path or test name
cargo clippy --workspace --all-targets # keep it clean
```

`game` takes a long time to build the first time because Bevy is large. `profile.dev` builds workspace code at
opt-level 1 and dependencies at opt-level 3.

Scripted runs drive the real game. They need a window. They panic if they don't see what they expect:

```bash
GAME_SCRIPT=tour GAME_SEED=7 GAME_SAVE=/tmp/fp-tour.ron cargo run -p game
```

- `tour` plays the chapter with typed expressions and writes screenshots to `shots/`.
- `walk` drives the avatar with key presses.
- `intro` photographs the intro, presses Start and checks the hand-over.

Scripts go through the same requests as the UI; they don't set results directly. New checks belong in
`game/src/devscript.rs` as `Step`s.

Environment variables:

| Variable | Default | Effect |
|---|---|---|
| `GAME_SAVE` | `saves/slot1.ron` | Save path |
| `GAME_SEED` | time-based | Seed for a new game |
| `GAME_SETTINGS` | `settings.toml` | Tutor settings file |
| `GAME_SHOTS` | `shots` | Screenshot directory |

Use a throwaway `GAME_SAVE` for scripted runs. A save that already exists overrides `GAME_SEED`.
`cargo run -p game --example render_check` renders one frame, which checks that the machine can render at all.

## Architecture

`docs/architecture.md` is the authoritative overview. `docs/tutor.md` covers Lumen, the AI tutor.
`docs/curriculum.md` and `docs/game-design.md` cover the design. The key ideas:

- **Three crates; only `game` uses Bevy.** `crates/physics_core` (units, calculator, simulators, puzzles,
  progress) and `crates/tutor` have no engine dependency and test in seconds. Nothing depends on `game`.
  Keep Bevy types out of the logic crates.
- **The tutor is read-only because of the dependency graph.** `tutor` depends on `physics_core` only for data
  types. It receives a plain `TutorContext` snapshot and returns text. The model gets no tools.
  - The only thing the game reads from a reply is a `[look: NAME]` gaze tag (`tutor::gesture`). NAME must be
    a place listed in the snapshot.
  - Replies pass through a leak filter (`tutor::filter`) that redacts numbers within 2 % of the private
    answers. It must also behave correctly while a reply is still streaming.
  - Don't add any other channel from the tutor back into the game.
- **Two physics layers.** The exact simulators in `physics_core` (`ballistics`, `demo`) decide every outcome.
  They are closed-form, independent of frame rate, and tested against analytic results. avian3d only
  illustrates. The exact simulation runs up to the judged event, records the verdict, and only then hands the
  body to the engine with its current velocity. Nothing the engine does afterwards can change a verdict.
- **One seed, one layout.** `puzzle::Ch1Layout::generate(seed)` produces the numbers the puzzles use. The island
  generator (`game/src/island.rs`) builds every puzzle-relevant distance from that same layout in whole
  blocks, so what the player measures with the ruler equals what the simulator uses. Changing a puzzle's
  geometry therefore touches both files. `game` tests check the island against the simulators' assumptions.
- **A puzzle is a kind plus content.** The kind is Rust code in `physics_core::puzzle`: generate numbers from the
  layout, judge an attempt, write the worked solution. The content is data in
  `assets/curriculum/tier1_ch1.ron`: titles, briefs, concept pages and where each is found, and experiment
  questions. A new puzzle of an existing kind is a data change; a new kind is a code change with tests.
  - The `.ron` is compiled in with `include_str!` (`content::chapter_1_1`), so editing it requires a rebuild.
    `content` tests validate it.
  - `state::PUZZLES` must match the chapter's puzzle count and the island's stations.
- **Game states.** `state::Mode` starts in `Menu`, where the intro plays behind the title menu
  (`intro.rs`). Its timeline lives as pure, engine-free functions in `cinematic.rs`, which is where it is
  tested. Start switches to `Playing`, and the avatar, stations, panels and Lumen run only there. Everything
  the intro spawned is despawned at the switch, except the mainland.
- **The game never blocks on the network.** `tutor_bridge` runs requests on a thread that reports through a
  channel, and the game polls it once per frame.
- Without a `[tutor]` section in `settings.toml` (see `settings.example.toml`), Lumen runs offline. The API key
  is read from the environment variable that `api_key_env` names. No model vendor is built in. The single
  adapter speaks the "chat completions" format.

## Conventions

- Rust edition 2024. Lines run long (about 120 columns). Lint exceptions use
  `#[expect(..., reason = "...")]` or `#[allow(clippy::too_many_arguments)]` on Bevy systems.
- Docs, doc comments and player-facing text use plain, concrete English. When behaviour changes, update the
  matching `docs/*.md` and the README.
- Commit subjects are short and imperative, and describe the change from the player's side (for example
  "Launchers show the blueprint before it is committed").
