# Notes for working on this repository

## Words the player reads or hears

Any text the player sees or hears follows [docs/writing.md](docs/writing.md): Lumen's voice, the
handbook's textbook voice, the briefs' plain instructions, and the habits to avoid in all of them.
Read it before you write or change any of these:

- chapter and tier files in `assets/curriculum/*.ron` (and any new ones)
- Lumen's lines in `game/src/companion.rs`
- Lumen's persona, `POLICY` in `crates/tutor/src/prompt.rs`
- hints and labels in `game/src/ui.rs`, `game/src/handbook.rs` and `game/src/carry.rs`
- the intro captions in `game/src/cinematic.rs`
- the station's refusals, results and blueprint lines in `crates/physics_core/src/puzzle.rs`, and the
  calculator's errors in `crates/physics_core/src/expr.rs` (the worked solutions there are the
  tutor's private notes and exempt)

`cargo test -p physics_core content` checks every chapter and tier file against the rules a program
can check.
Lines written in Rust aren't checked automatically, so hold them to the same rules by hand.

## Design

Before you change how the game plays, read `docs/game-design.md` (the design and the story),
`docs/curriculum.md` (the chapters and puzzles), `docs/architecture.md` and `docs/tutor.md`.
