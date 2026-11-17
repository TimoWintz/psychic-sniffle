# Curriculum

Ordered by difficulty, not by domain. Three tiers; each tier visits mechanics (M), electromagnetism (E)
and thermodynamics (T) at a deeper level. A chapter unlocks when its prerequisites are complete.

Each chapter has 4 to 6 puzzles and ends in a capstone that installs a ship part. Only tier 1,
chapter 1 is built in the slice; its tutorial and four puzzles are specified at the end of this document.

Reading the tables: **Maths** is the hardest mathematics the chapter needs. **Needs** lists
prerequisite chapters.

## Tier 1 — Skiff (secondary school; algebra)

The skiff crosses the strait to the main hull, where the rest of Lumen is; each chapter adds the part
the crossing needs (see "The story" in the game design).

| # | Chapter | Concepts | Maths | Needs | Ship part | Example puzzles |
|---|---|---|---|---|---|---|
| 1.1 | M — The Salvage Launcher | position, velocity, relative velocity, acceleration; free fall; projectile motion | quadratics, basic trigonometry | — | skiff frame | time a crawler and catch it on a marker; measure local gravity from a timed drop; launch a crate across a canyon; clear a ridge and hit the pad |
| 1.2 | E — First Light | charge, current, voltage, resistance; Ohm's law; series and parallel; electrical power | linear equations | — | lighting and power loop | size a resistor so a lamp runs at its rating; voltage drop along a long block wire; three lamps, one supply: series or parallel?; choose a fuse |
| 1.3 | M — The Haul | Newton's three laws; weight; normal force; static and kinetic friction; inclines | vectors in components | 1.1 | haul winch, hull plates delivered | mass of a block stack from density; friction coefficient from the tilt angle at which a sled slips; winch tension up a ramp; pull needed to cover a distance in a set time |
| 1.4 | T — The Forge | temperature; heat; specific heat; mixing; latent heat | linear equations | 1.2 | hull alloy | energy to bring an ingot to forging temperature; final temperature of a quench bath; energy to melt a charge; heating time with an electric element |
| 1.5 | M — The Lift | work; kinetic and potential energy; conservation; power; efficiency | quadratics | 1.3 | skiff drive | counterweight for an elevator; spring compression for a launch speed; motor power to raise a load in a set time; braking distance |
| 1.6 | Test flight — Skiff Trial | all of tier 1 together | — | 1.1–1.5 | skiff complete | thrust for the required acceleration at the built mass; battery energy for the crossing; motor temperature must stay under its limit |

## Tier 2 — Orbiter (upper secondary to early university; vectors, trigonometry, logarithms, first calculus)

| # | Chapter | Concepts | Maths | Needs | Ship part | Example puzzles |
|---|---|---|---|---|---|---|
| 2.1 | M — The Gantry | torque; equilibrium; centre of mass; tension at angles | vector components | 1.6 | orbiter hull frame | counterweight a crane arm; centre of mass of a block assembly; cable tensions; landing legs that do not tip |
| 2.2 | M — Staging | momentum; impulse; collisions; thrust; rocket equation | logarithms | 2.1 | thruster stack | recoil of a cart; docking speed after an inelastic join; thrust from mass flow; propellant for a required change in velocity; two stages against one |
| 2.3 | E — The Capacitor Bank | Coulomb's law; electric field; potential; capacitance; stored energy | inverse-square laws | 1.6 | ignition system | levitate a charged dust grain; deflect a particle between plates (reuses 1.1); capacitors in series and parallel; energy for one ignition pulse |
| 2.4 | T — Pressure Hull | pressure; ideal gas law; internal energy; first law; isothermal and adiabatic processes | exponents, area under a curve | 1.6 | life support tanks, compressor | tank pressure at noon temperature against its limit; amount of air for the cabin; work to compress; read a pressure–volume diagram |
| 2.5 | M — Reaction Wheels | angular kinematics; moment of inertia; angular momentum; rolling | sums over blocks | 2.1 | attitude control | spin-up time under a known torque; moment of inertia of a block wheel; turn the hull by spinning a wheel the other way |
| 2.6 | E — The Generator | magnetic force on charges and currents; flux; Faraday and Lenz; motors, generators, transformers | rates of change | 2.3, 2.5 | power plant | force on a current-carrying rail; radius of a particle's path in a field (isotope separator); voltage from a rotating coil; transformer turns ratio |
| 2.7 | T — Heat Shield | conduction; thermal resistance; convection; radiation; Newton's law of cooling | exponentials | 2.4 | heat shield, radiators | heat flow through a layered wall (same mathematics as 1.2); radiator area to reject a load; cooling time; insulation thickness |
| 2.8 | M — Orbit | simple harmonic motion; pendulum; circular motion; gravitation; circular orbits | square roots, proportionality | 2.2 | dampers, flight computer I | period of a mass on a spring; re-measure gravity with a pendulum; orbital speed and period at a chosen height; a relay that stays overhead |
| 2.9 | Test flight — Orbital Insertion | all of tier 2 together | — | 2.1–2.8 | orbiter complete | velocity budget across stages; power budget in sunlight and shadow; thermal balance in orbit |

## Tier 3 — Starship (first-year university; calculus, differential equations)

| # | Chapter | Concepts | Maths | Needs | Ship part | Example puzzles |
|---|---|---|---|---|---|---|
| 3.1 | M — Spin Gravity | work by a varying force; moment of inertia by integration; rigid-body dynamics | integration | 2.9 | habitat ring | work along a path with a varying force; inertia of a continuous ring and spokes; spin rate and radius for comfortable weight |
| 3.2 | E — The Magnetic Nozzle | Gauss's law; Ampère's law; fields of symmetric sources; solenoids | flux and line integrals | 2.9 | ion engine | field of a charged plate, wire and shell; solenoid current for a required field; confine a beam |
| 3.3 | M — Resonance | damped and driven oscillation; resonance; travelling and standing waves; Doppler shift | second-order differential equations | 3.1 | hull tuning, ranging sonar | choose damping to settle in time; move a natural frequency away from the engine's; standing waves in a fuel line; closing speed from a shifted echo |
| 3.4 | E — The Tuner | RC and RL transients; LC oscillation; impedance; RLC resonance; power factor | first-order differential equations, complex numbers | 3.2, 3.3 | radio | time constant for a delay; tune to a carrier frequency; match a load |
| 3.5 | T — The Reactor Cycle | second law; entropy; Carnot limit; engine and refrigeration cycles | integration, logarithms | 2.9 | main power cycle, cryogenic plant | best possible efficiency between two temperatures; net work of a cycle; entropy change of mixing; refrigeration for stored propellant |
| 3.6 | M — Transfer Window | orbital energy; elliptical orbits; Kepler's laws; transfers; escape | conservation laws with calculus | 3.1 | navigation computer | speed at any point of an ellipse; a two-burn transfer; escape speed; when to leave |
| 3.7 | E — The Antenna | displacement current; Maxwell's equations; electromagnetic waves; intensity; radiation pressure | wave equation | 3.4 | deep-space communication, light sail | wave speed from the two field constants; antenna length for a frequency; received power at a distance; force of sunlight on a sail |
| 3.8 | T — Fuel Lines | kinetic theory; speed distribution; mean free path; continuity; Bernoulli; viscous flow | statistics, integration | 3.5 | propellant feed | molecular speeds at tank temperature; flow speed through a narrowing; pressure drop along a line |
| 3.9 | Finale — Departure | everything | — | 3.1–3.8 | starship complete | mass, velocity, power, heat and communication budgets must all close |

## Cross-links worth keeping

These are deliberate reuses, flagged so that later chapter design does not lose them.

- 1.1 kinematics returns in 2.3 (a charged particle between plates is a projectile).
- 1.2 series and parallel returns in 2.3 (capacitors) and 2.7 (thermal resistance).
- 1.5 energy returns in 2.8 (orbital energy) and 3.6.
- 2.8 simple harmonic motion returns in 3.3 (mechanical) and 3.4 (electrical): one equation, two worlds.
- 2.4 first law returns in 3.5.

## A possible tier 4

Special relativity, an introduction to quantum physics, and statistical mechanics fit the same pattern
(a fourth ship stage, all domains advancing together). Not designed here.

---

## Slice specification: 1.1 The Salvage Launcher

Planet gravity `g` is drawn per save from 6.5 to 13.5 m/s², excluding 9.3 to 10.3 so that the familiar
value never works. All lengths are whole metres so they can be checked by counting blocks.

The stations are the machines of the ancient way (see the game design's story): asleep at the start,
woken one by one as each job is done right. The builders give no numbers; every orange mark is the
castaway's. Each puzzle's **Wakes** line says what its pass wakes (see `game/src/waking.rs`).

### Puzzle 0 — The supply rail (tutorial)

- **Story.** The first crate out of the wreck holds the castaway's tools; nothing hard, this is how
  every station works. Pass: the marker lit when the cart stopped on it, and Lumen did nothing.
- **Wakes.** The marker's quartz, then the stone gate at the end of the stretch, which opens; the cart
  goes on through as the haul cart.
- **Purpose.** Teach the station loop before any physics: brief, ruler, notebook, blueprint, units,
  commit, result, charges. It needs no mathematics; the answer is the measurement itself.
- **Brief.** The builders' cart, on a straight, level rail sunk into the road beside the wreck,
  drives exactly as far as it is told and stops. Load it, then send it to the orange marker.
- **Carrying.** The cart starts empty and will not run so. Its cargo crate stands beside the rail
  start; the player picks it up (`F`) and puts it on the cart (`F`), which teaches the verb the
  canyon needs, with nothing at stake. Committing with an empty cart is refused for free ("load the
  cart first"), before anything typed is read, so it is not the first commit that writes the page.
  The crate rides with the cart on every run and stays on it; it can be taken off again and put
  back down beside the rail, and the save keeps whether it is loaded.
- **Investigate.** Ruler: the cart's starting point to the marker centre (both pinned, on the near
  side of the rail so the cart does not hide them). The distance is a whole number of metres, 9 to 17,
  drawn after every other number so existing saves keep their layout.
- **Predict.** Enter the run distance: the notebook name (`d1`) or the number with its unit. A bare
  number is refused with the reason and costs no charge, which is the lesson about units.
- **Pass.** The cart stops on the marker (one block, so centre ± 0.5 m). Too short or too far is
  reported in metres. The rail is 24 m long; longer runs are refused.
- **Gates.** Puzzle 1 requires it. (Puzzle ids are `1.1.0` to `1.1.4` in this order; saves made before
  the haul track was added used other ids and are not read.)
- **First page.** The first value committed here with the cart loaded, tested or refused, writes
  *Measurements carry units*: the handbook is met in the tutorial.

### Puzzle 1 — Catch the crawler

- **Story.** The crawler is the builders' own, still walking its rounds, and it carries the sight on
  its back: the piece the launchers aim with. Pass: the sight is on the crane, and the launchers can
  aim once they know how hard the planet pulls.
- **Wakes.** The transfer crane's arm, which lifts the sight off; then the light runs to the tower.
- **Purpose.** Velocity and relative velocity, before any acceleration: textbooks start here, and the
  drop tower's strobe ("evenly, or further apart?") then has a steady motion to be compared with.
- **Brief.** The builders' crawler walks the haul track round after round; from its mark at the end
  of a round it sets off at one steady speed, and cannot be stopped. The haul track is the rail's
  stretch past the stone gate, and the haul cart is the tutorial's cart, gone on up the line: it
  stands behind the crawler on the same track and runs at whatever speed is set. When the cart reaches the crawler the two couple; the load
  can only be lifted off at the transfer marker. Set the cart's speed so it reaches the crawler there.
- **Investigate.** Ruler: the cart's front coupler to the crawler's rear coupler (the head start
  `D`), the cart's front coupler to the transfer marker's centre (`M`), and timing post A to timing
  post B (`s`). Pins for the couplers and the marker sit on the near side of the track; the posts'
  pins are on top of them. All are whole metres. Stopwatch: every run of the experiment times the
  crawler from post A to post B and notes it (`t1`, ...), to 0.01 s. The crawler's speed is written
  nowhere; it is worked out from `s / t`.
- **Try it first.** "The crawler and the haul cart leave their marks together, the cart at a lower
  steady speed. A flash marks both at equal moments. How does the gap between them change?" Grows by
  the same amount every moment / grows faster and faster / stays the same. The cart runs at half the
  crawler's speed, away from the answer. Both leave evenly spaced afterimages. Writes *Velocity* and
  *Relative velocity*.
- **Predict.** Enter the cart's speed. The cart is rated to 8 m/s (on the station); faster, or zero,
  is refused for free.
- **Test.** Both set off from their marks together and keep their speeds. The cart reaches the crawler
  at `t = D / (v − u)`, `x = v D / (v − u)`; the crawler stops at the buffer at 40 m.
- **Pass.** The cart reaches the crawler on the marker (one block, so centre ± 0.5 m). A miss says
  where, in metres before or past the marker centre, or that the crawler reached the buffer still
  ahead of the cart. The result panel draws both runs as distance against time: two straight lines
  that cross where the cart reached the crawler.
- **Relations.** `u = s / t`; the gap closes at `v − u`; `v = u M / (M − D)`.
- **Design of the numbers.** The crawler runs at 1.2 to 2.4 m/s, the posts are 10 to 14 m apart (so
  the stopwatch reads 4 s or more, good to about 0.12 %), the head start is 5 to 10 m and the marker
  22 to 32 m out. Draws are kept only where the speeds that pass reach at least 0.6 % either side of
  the one needed, five times the stopwatch's error, so the player's own `u` always does. These numbers
  are drawn after all the others, so the rest of the island stays as it was for a seed.
- **Guessing.** A guesser who halves the cart's range on the side each miss points to needs 6.4 tests
  on average and is done within three in 6 % of saves. A test in `puzzle.rs` keeps it so.
- **Gates.** Puzzle 2 requires it.

### Puzzle 2 — Local gravity

- **Story.** The launchers do exactly what they are told and work nothing out, and Lumen's backup only
  knows home's gravity: Lumen would rather have the player's number than its guess. Pass: the
  player's g is in Lumen, and every later shot is worked out with it.
- **Wakes.** The tower, to the top; the ramp onto the outcrop surfaces, and the sight is set on the
  ledge launcher.
- **Brief.** The launchers need this planet's gravitational acceleration.
- **Investigate.** Measure the drop tower's height with the ruler. Release the test mass and time the
  fall with the stopwatch (free, repeatable).
- **Place to measure: the light gate.** A gate on a stand by the station, carried (`F`) and clamped on
  the drop tower at any whole-metre notch (painted orange by the castaway), from 1 m below the
  release down to the plate: the notch the pointer marks on the tower, or the one at hand height. With a gate fitted, each release times
  the fall from the release to the gate and notes the pair under one number (`h2`, `t2`, then `h3`,
  `t3`, ...), so the player designs their own measurement and can see that doubling the drop does not
  double the time. Without one, the stopwatch times the whole drop. The gate's notch is kept in the
  save; a ruler pin rides on it. The answer stays the typed `g`.
- **Short drops are worth less.** Readings are rounded to 0.01 s, so at 1 m (about 0.4 to 0.55 s)
  the rounding alone is up to about 2.6 % of `g`, beyond the 2 % pass. A reading of a second or more
  (`h ≥ g/2`, 3.3 to 6.8 m) gives `g` to 1 %. The station's text says only that the longer the drop,
  the better the reading; players find the rest by fitting several notches. Tests hold every notch
  that times a second or more to 1 %, and a straight line through all the pairs (`h` against `t²`) as
  well. A plot of `h` against `t²` belongs to the grapher (tier 2), not here.
- **Try it first.** "How will flashes at equal moments be spaced?" Evenly / further apart lower down /
  closer together. The release is the experiment: the flashes leave afterimages and ticks whose
  gaps grow. Writes the pages *Acceleration* and *Free fall*.
- **Predict.** Enter `g`.
- **Test.** A second mass is released from the taller survey mast. The station shows the fall time
  predicted from the player's `g` beside the measured one.
- **Pass.** Within 2 % of the true value. The player's `g` is then written to the notebook and used by
  the later puzzles — their own measurement, not the game's number.
- **Relation.** `h = ½ g t²`.

### Puzzle 3 — Across the canyon

- **Story.** The frame beam itself, packed in a salvage crate, crosses the canyon: the first real
  shot with the player's g. The platform is beside the tilting launcher. Pass: the crate stays on the
  platform and the beam waits on the tilting launcher.
- **Wakes.** Both rims; the seal across the road at the ridge's end sinks.
- **Brief.** Land a salvage crate on the strip across the canyon. The ledge launcher fires
  horizontally, but its worn dial turns only within a band (`vmin` to `vmax`, found and painted on
  the rating plate by the castaway), and from the bare ledge no speed in the band reaches the strip.
  The launcher rides in a stone jack frame at the rim; every crate slid into its sockets raises it
  one metre.
- **Investigate.** Ruler: horizontal distance from the release point to the strip's centre, and the
  drop to it (the release point's pin rises with the launcher). The rating plate on the station gives
  the band, with a button to note it (`vmin`, `vmax`).
- **Try it first.** "One crate dropped, one fired sideways, at the same moment: which reaches the
  canyon floor first?" Both at once. The test crates leave from wherever the launcher stands. The fired
  crate is slow enough to land mid-canyon; both fall the launcher's height plus the canyon's depth, not
  the drop to the strip, so the time is not the puzzle's. Writes *Horizontal and vertical motion are
  independent*.
- **Build, then compute.** Work out which release heights the band allows, carry crates (`F`) from
  the pile of eight on the ledge into the frame, then type the speed for the height built. The
  blueprint shows the frame's crates beside the typed speed, and the save keeps them. A speed outside
  the band is refused before launch and costs nothing, so a wrong stack shows up for free once the
  speed for it has been worked out.
- **Pass.** The crate lands on the strip (one block long, so centre ± 0.5 m).
- **Relations.** `v = d / sqrt(2H/g)` for a release height `H`; the band allows
  `g d² / (2 vmax²) ≤ H ≤ g d² / (2 vmin²)`; crates = `H −` the ledge's height, in whole metres.
- **Design of the numbers.** The ledge is 3 to 5 m above the strip and the strip 22 to 42 m out. The
  band is placed halfway between the speeds that neighbouring stacks need, so that one or two stacks
  between 1 and 6 crates are inside it, and no stack's speed is within 1.5 % of either end: a
  player's own `g` (good to about 0.5 %) never tips a stack in or out. Both ends are rounded to
  0.1 m/s on the plate. The longest shot the dial allows, from a full frame, still comes down on the
  flat ground beyond the canyon. These numbers are drawn after all the others, so the rest of an older
  save's island stays as it was.
- **Guessing.** A player who guesses but uses every miss well (halving the band on the side the miss
  points to, and moving a crate when it runs out) needs 7.6 tests on average and is done within three
  in about a third of saves. The old typed-speed canyon took 3.9 (a third within three); the first crate
  version, with a fixed speed, 2.8 (three quarters within three). A test in `puzzle.rs` keeps it so.

### Puzzle 4 — Over the ridge (capstone)

- **Story.** The one that matters: the frame beam, come across the canyon, over the ridge to the
  shipyard on the builders' old quay on the north shore.
- **Wakes.** The landing circle, then the north gate, whose light points across the strait at the main
  hull. In the closing, Lumen hears the core for the first time: faint, but there.
- **Brief.** Launch the frame beam over the ridge onto the shipyard pad. The launcher has a maximum
  speed.
- **Investigate.** Ruler: distance to the pad, distance to the ridge, height of the ridge.
- **Try it first.** "Two slugs at the same speed, 30 and 60 degrees: which lands further away?" The
  same place, at 0.6 of the distance to the ridge, well short of it. Writes *Splitting a launch into
  components* and *Range on level ground*.
- **Predict.** Enter the launch angle and the launch speed.
- **Pass.** The beam passes over the ridge without touching it and lands on the pad (5 m long, so
  centre ± 2.5 m). The tracked point is the beam's underside; the brief tells the player to leave margin.
- **Design of the numbers.** The ridge is placed so that 45° does not clear it, and the speed limit
  rules out very steep shots, leaving a window of roughly ten degrees. The player must reason about
  both constraints before computing the speed.
- **Relations.** `x = v cosθ · t`, `y = v sinθ · t − ½ g t²`; on level ground `R = v² sin 2θ / g`.
- **Reward.** The skiff's frame rises on the shipyard floor, at the top of the old slipway.

### Handbook pages: Earth pictures and quizzes

Each page of the chapter shows a looping picture of its idea set on Earth, never on the island, and
has an optional quiz (two or three multiple-choice questions; Earth's numbers only). A right answer
explains itself; every question right ticks the page. Nothing in the world depends on it.

| Page | Picture | Quiz |
|---|---|---|
| *Measurements carry units* | a tape measure along a sofa: "2" against "2 m" | which value is a time; a bare number refused; km to m |
| *Velocity* | a cyclist past distance posts, strobed each second: equal steps | distance over time; evenly spaced photos; `x = x0 + v t` |
| *Relative velocity* | a car catching a bus, with the gap at each second shrinking by equal steps | same way: the difference; towards: the sum; time to close a gap |
| *Acceleration* | a car pulling away from traffic lights, strobed each second: the gaps grow, and its speed at each second rises by equal steps | speed readings to `a`; change in `v` over time; `x = ½ a t²` from rest |
| *Free fall* | an apple from a balcony, strobed (gaps grow); a feather and a hammer in a tube with no air | strobe spacing; mass does not matter; `h` after 2 s with Earth's g |
| *Horizontal and vertical motion are independent* | a marble rolled off a table and one dropped, level at every flash (shown slowed) | which lands first; horizontal speed in flight; `x = vx t` |
| *Splitting a launch into components* | a football kick with its `vx`, `vy` and `v` arrows; `vy` shrinks and turns, `vx` stays | steeper means more `vy`; `vx` at 90°; `vy` at 30° |
| *Range on level ground* | two kicks at 30° and 60° landing on one flag | the best angle; the partner angle; which stays up longer |

## Planned: 1.3 The Haul, place the catcher

Direction only; chapter 1.3 has no puzzles in code yet. It is the third "doing it with your hands"
pattern (see the game design): the setting is fixed, and the answer is where the player carries the
target.

- A sled is pushed onto a level run (or a ramp) by a fixed launcher, and friction brings it to rest.
  The push is read off a plate. The player places the stop block where the sled will stop, in a row
  of slots along the run.
- It needs a friction simulator in `physics_core`, tested against closed form; the row of slots, as
  another `carry::Slot` (the cart, the tower's notches and the jack frame are the ones built); and a
  guesser test (see "What makes guessing a bad strategy").
- A row of slots alone is a handful of options, the trap the first crate canyon fell into: a guesser
  found it in 2.8 tests. So make the slots fine and the target tight, or combine the placement with a
  calculated value: the friction coefficient, measured first from the tilt at which the sled slips.
