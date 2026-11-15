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

Lumen is on screen as a holographic eye in the lower left corner. It points with a beam of light only
where that helps: the way to the next station for a while after it opens (and again when you stand at
a station that is done or locked), the experiment and the job of an unsolved station (and what to
carry for it: the cargo crate, the light gate, the ledge's crates, then where they go), the yellow pin
to click while measuring, the target while a test runs. Nothing about a solved puzzle draws the beam;
otherwise the eye just looks about. When the place is off screen, the beam ends in an arrow at the
edge of the screen. It says short lines in a bubble above the eye when something happens (a test lands,
a station refuses an entry, a new station opens) and its eye shows a mood: pleased, concerned,
thinking. Talking to it (`T` or a click on the eye) opens the conversation where the bubble appears,
attached to the eye. None of this needs a language model; with one connected, Lumen can also turn its
eye to a place it is talking about.

### The look of the world

Alien, but familiar. The planet has turf, rock, sand and trees, only not quite ours: the turf is teal
with drifts of olive and patches of violet lichen, the rock is slate-violet laid down in rust and pale
strata, the trees are parasols and spires of teal and violet fronds, and quartz grows in clusters that
glow. A ringed gas giant and a small moon hang in a turquoise sky, and other islands sit in the haze
on the horizon.

Puzzle sites are built by the castaway from the wreck, so they share one look: hull plates laid on the
turf around each station, salvage crates, quartz lamp posts, worn paths from one to the next, and
targets painted survey orange (they glow) with staked corners. Orange means "a number lives here".
The landforms a puzzle needs (the outcrop, the canyon, the ridge) are geology, not boxes: exact where
the puzzle measures them, natural everywhere else.

## Title and intro

The game opens on a title menu (for now only Start). Behind it the intro plays on a loop, in attract
mode: how the castaway came to be here, going back through the levels from the last to the first,
seen from above.

| | Level | What happens | Set |
|---|---|---|---|
| III | Orbit | The ship comes in too fast and breaks up. Its drive section tears loose and is left drifting. | placeholder: a planet from orbit |
| II | The mainland | The main hull comes down on the far side of the strait. | placeholder: a landmass with the main wreck |
| I | The island | The forward section crosses the strait and dives into the wreck site; the castaway climbs out. | the island of chapter 1.1 |

Played backwards, the levels are the way the castaway arrived: out of orbit, over the mainland,
onto the island. The game then plays them forwards, island first. Start hands over at once: the
camera glides down to the player's view and Lumen says its first line. The placeholder sets are swapped for the real levels as they are built.

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
   Each physics station also has an experiment to **try first**. It shows the idea the puzzle rests
   on: a strobe on the crawler and the haul cart pulling apart at steady speeds, a strobe on the falling
   test mass, a crate dropped beside one fired off the ledge, two shots
   at complementary angles. The player picks what they expect from a few choices (no numbers), then
   watches. Experiments are free and repeatable and give no answer away. Watching one writes its
   pages into the handbook.
3. **Predict.** Enter values, with units, in the blueprint panel. The calculator accepts expressions
   over notebook names (`(d1 + d2) / 2`, `sin(30 deg)`). Examples in the interface show how to write
   an expression, never which relation to use.
4. **Test.** Commit. The simulation runs in the world; the result panel shows what was predicted
   against what happened, with a graph.
5. **Result.** Pass: the part is fabricated and appears on the ship. Fail: a diagnostic says what
   happened in physical terms ("landed 3.2 m short"), never what the answer is.

### Doing it with your hands

Typing a number is not the only way to commit. Some puzzles are answered, or investigated, by moving
things in the world. Three patterns, each with the same rule: the player works it out first, then
builds it, and the exact simulator judges what was built.

| Pattern | What the player does | Where |
|---|---|---|
| Build, then compute | Work out what the world must hold, build it, then calculate the rest for what was built: crates under a launcher whose dial only turns within a band set its height, and the speed for that height is typed. | 1.1 Across the canyon (built) |
| Place to measure | Put an instrument where it measures what is wanted: a light gate clamped at a chosen notch of the drop tower, so the player designs their own measurement and collects pairs of drop and time. | 1.1 Local gravity (built) |
| Place the catcher | The setting is fixed and read off a plate; carry the target (net, stop, bin) to where the load will end up. | 1.3, the sled: place the stop where it comes to rest under friction (planned; see the curriculum) |

The verb is taught first with nothing at stake: the tutorial's cart will not run until the player
has carried its cargo crate onto it. That refusal is free, like a value of the wrong kind, and since
nothing typed was committed it does not write the tutorial's handbook page.

Placements snap to whole blocks or to fixed notches, so what is built can be counted and measured
exactly. A placement on its own is a small choice (a handful of stacks), which a few tries find by
luck, so a built answer comes with a calculated one or a small target: guessing should cost at least
as many tests as guessing a typed value, and every wrong stack also costs a walk with crates. An
instrument that is placed (the light gate) answers nothing itself; what it measures still has to be
worked into the typed answer.

### What makes guessing a bad strategy

- **Measure how fast a guess finds the answer.** Every miss says which way it went (short or
  beyond, sooner or later), so any answer can be found by halving after each miss. How likely a
  lucky pass is, is really how many tests that halving takes: the size of the target against
  everything the station accepts, in bits. Every answer that is built in the world gets a test that
  plays such a guesser against it across many seeds: it must need well over a station's charges on
  average, and rarely pass within them. At the canyon the same guesser needed 3.9 tests (31 % within three) with the
  old typed speed, 2.8 (75 %) with crates alone at a fixed speed, and 7.6 (32 %) now that the stack is
  built and the speed is calculated for it. The haul track's typed speed has one too: a marker one block
  wide, a coupling point that moves fast with the speed, 6.4 tests (6 % within three).
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
- **Handbook.** Materials table and the concept pages found so far (definitions and relations,
  no worked answers). A page is written when the player sees what it describes happen: an
  experiment, or for the first page, the tutorial's first commit. Until then it is listed as locked,
  with where to find it. Lumen announces new pages, each station has buttons for the pages its job
  uses, and after a miss Lumen names the page worth rereading.
- **Launchers show the blueprint.** As a value is typed, the tilting launcher's arm swings to the
  angle and its gauge fills to the speed, against the launcher's rating. The ledge launcher's gauge
  marks the bottom of its band too and turns red outside it, and the launcher itself rides up its jack
  frame as crates go in.
  The player's choice is visible in the world before it is committed; the path is never previewed.
- **Grapher** (tier 2 onward). Plot a notebook series; read slopes and areas.

## Avatar, camera, controls

Third-person view of a blocky engineer. Walk, jump, look around; `E` uses a station; `F` picks up
and puts down; `R` measures with the ruler; `N` notebook and calculator; `H` handbook; `T` (or a click
on Lumen's eye) talks to Lumen. The avatar carries one thing at a time, in its hands; there is no
inventory.

- **Stations.** While a station is open the engineer stays at its console: the movement keys do
  nothing, the camera still turns and zooms. `E`, `Esc` or the panel's Leave button steps away.
- **Ruler.** `R` takes one measurement: two clicks, then the ruler stops taking clicks and its reading
  stays on screen to be noted. `R` again measures anew; `Esc` or the card's close button puts it
  away; `R` or `Esc` before the second click cancels.
- **Carrying.** Salvage with an orange handle can be moved. `F` picks up what is in front of the
  avatar; a ghost block shows where it will go, and `F` puts it down there: on the ground, or on top
  of a crate, at most one block above the feet (so crates can be stacked into steps). Crates are real
  blocks: they can be stood on and measured, and each stays near where it belongs (the ledge's
  salvage on the ledge, the cart's cargo beside the rail). Where a station reads a placement,
  putting the thing into its place is the act: a crate onto the cart or into the jack frame, the
  light gate onto a notch of the drop tower (the notch is the one the pointer marks on the tower, or
  the one at hand height). The station shows what is placed there, and the save keeps it.
- **Notebook, handbook, Lumen.** Always on screen. The notebook and the handbook sit in a fixed column
  at the top left under the chapter's progress, reduced to a header; `N` and `H` (or a click on the
  header) expand and reduce them. Lumen reduced is its eye; `T` expands its conversation.

The engineer is a jointed figure of blocks, animated in code. Its stride follows its speed, it
tucks on a jump and gives at the knees on landing, and it works the panel of an open station. It carries a
crate in both hands, arms forward. It turns to watch a test in flight, then cheers a pass or hangs its
head at a miss. When idle, it glances where
Lumen is pointing.

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

Mining or free building (only marked salvage moves, and only where a puzzle has room for it), inventory, multiplayer, sound, saving more than chapter progress, tiers 2 and 3
content, the relay server for the tutor.
