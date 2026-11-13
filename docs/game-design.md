# Game design

Working title: **First Principles**.

## Vision

You are stranded on a small voxel planet beside the wreck of your ship. To leave, you build a new ship
three times over — a skiff, an orbiter, a starship — and every part of it has to be earned by doing the
physics that the part depends on. By the time the starship leaves, the player has worked through
mechanics, electromagnetism and thermodynamics to first-year university level.

The ship's computer, **Lumen**, survived the crash with its sensors intact and its actuators gone. It
can see what you are doing and explain anything, but it cannot press a single button. Doing is the
player's job.

## Pillars

1. **The world is the textbook.** One voxel is one cubic metre of a real material. Distances, masses,
   resistances and heat flows in a puzzle are properties of the world the player can measure, not
   numbers printed in a problem statement.
2. **Commit, then find out.** The player works out an answer, commits it, and the world runs. Nothing is
   marked right or wrong by a teacher; the crate either lands on the platform or it does not.
3. **Understanding beats guessing.** The game is built so that calculating is the fast route and trial
   and error is the slow, expensive one.
4. **The tutor explains, the player acts.** Lumen never solves a puzzle and has no way to touch the game.
5. **Difficulty rises evenly.** All three domains advance together, tier by tier; no domain reaches
   university level while another is still at its first lesson.

## The world in SI units

- One voxel = 1 m × 1 m × 1 m of a named material.
- Every material has real properties, listed in the in-game handbook: density, specific heat, thermal
  conductivity, electrical resistivity. The handbook is the same table the simulators read.
- The planet's constants are generated per save. Gravity is not 9.81 m/s²; the first puzzle is to
  measure it. Later tiers do the same for atmosphere, day length and the star's output.

How each domain uses the voxels:

| Domain | What the voxels give |
|---|---|
| Mechanics | Mass = block count × density. Centre of mass, torque and balance are things you build. |
| Electromagnetism | A wire is a path of blocks with resistance from its material, length and cross-section. Circuits obey Ohm and Kirchhoff; they are not on/off logic. |
| Thermodynamics | Heat diffuses block to block. A heat shield is a wall whose material and thickness decide the outcome. |

## Core loop

Every puzzle runs through the same five steps at a **station** (a workbench in the world).

1. **Brief.** What the ship part must do, as a specification with tolerances.
2. **Investigate.** Measure the world. Readings go to the notebook under a short name (`h1`, `t1`).
   Experiments in this step are free and repeatable.
3. **Predict.** Enter values, with units, in the blueprint panel. The calculator accepts expressions
   over notebook names (`sqrt(2*h1/g)`).
4. **Test.** Commit. The simulation runs in the world; the result panel shows what was predicted
   against what happened, with a graph.
5. **Result.** Pass: the part is fabricated and appears on the ship. Fail: a diagnostic says what
   happened in physical terms ("landed 3.2 m short"), never what the answer is.

### What makes guessing a bad strategy

- **Commit before test.** Values cannot be adjusted while the simulation runs.
- **Prototype charges.** Each test consumes one charge; a puzzle starts with three. At zero the station
  needs recharging (a short wait in the slice; a resource errand later). Never a hard lock.
- **Seeded parameters.** Distances, heights and planet constants differ per save. Answers cannot be
  looked up or shared.
- **Dimension check is free.** An answer with the wrong dimension (a time where a speed is needed) is
  refused with an explanation and costs nothing. This teaches unit discipline without punishing it.
- **Quality.** A first-test pass makes a pristine part; later passes make a serviceable one. Part
  quality feeds the tier's final test flight (margins are tighter with serviceable parts).

### Instruments

Unlocked as the curriculum needs them.

| Instrument | Reads | From |
|---|---|---|
| Ruler | horizontal and vertical separation of two points | tier 1 |
| Stopwatch | duration of an experiment | tier 1 |
| Scale | mass | tier 1 |
| Multimeter | voltage, current, resistance | tier 1 |
| Thermometer | temperature | tier 1 |
| Force gauge, tachometer, pressure gauge | force, angular speed, pressure | tier 2 |
| Field probe, oscilloscope | field strength, signals over time | tier 2 and 3 |

### Player tools

- **Notebook.** Every reading, named, with its unit and where it came from. Entries are variables in
  the calculator.
- **Calculator.** Unit-aware. `12 m / 1.5 s` gives `8 m/s`; `3 m + 2 s` is an error that says why.
- **Handbook.** Materials table and the concept cards unlocked so far (definitions and relations,
  no worked answers).
- **Grapher** (tier 2 onward). Plot a notebook series; read slopes and areas.

## Avatar, camera, controls

Third-person view of a blocky engineer. Walk, jump, look around; `E` uses a station; `R` toggles the
ruler; `N` notebook and calculator; `H` handbook; `T` talks to Lumen. The avatar carries no inventory in
the slice.

## The ship and progression

The shipyard pad is visible from most of the island. Each chapter's capstone adds a part to the ship
standing on it, so progress is always in view.

| Stage | Purpose in the story | Unlocks |
|---|---|---|
| Skiff | cross the island and the strait to the main wreck | tier 2 stations and salvage |
| Orbiter | reach orbit, recover the drifting drive section | tier 3 stations |
| Starship | leave the system | ending |

Each tier ends with a **test flight** that needs all three domains at that level: a mass budget, a power
budget and a heat budget that must all close at once.

Chapters unlock by prerequisite, not in a fixed line: finishing a chapter opens every chapter whose
prerequisites are now met, so there are usually two or three open at once.

## Failure and tone

Failure is information. A failed test shows the physical outcome and leaves the experiment standing so
the player can look at it. Lumen's tone is a colleague's: curious, brief, never condescending, and
honest when the player is wrong.

## Out of scope for the slice

Mining or free building, inventory, multiplayer, sound, saving more than chapter progress, tiers 2 and 3
content, the relay server for the tutor.
