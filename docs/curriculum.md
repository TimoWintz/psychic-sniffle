# Curriculum

Ordered by difficulty, not by domain. Three tiers; each tier visits mechanics (M), electromagnetism (E)
and thermodynamics (T) at a deeper level. A chapter unlocks when its prerequisites are complete.

Each chapter has 4 to 6 puzzles and ends in a capstone that installs a ship part. Only tier 1,
chapter 1 is built in the slice; its tutorial and three puzzles are specified at the end of this document.

Reading the tables: **Maths** is the hardest mathematics the chapter needs. **Needs** lists
prerequisite chapters.

## Tier 1 — Skiff (secondary school; algebra)

| # | Chapter | Concepts | Maths | Needs | Ship part | Example puzzles |
|---|---|---|---|---|---|---|
| 1.1 | M — The Salvage Launcher | position, velocity, acceleration; free fall; projectile motion | quadratics, basic trigonometry | — | skiff frame | measure local gravity from a timed drop; launch a crate across a canyon; clear a ridge and hit the pad |
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

### Puzzle 0 — The supply rail (tutorial)

- **Purpose.** Teach the station loop before any physics: brief, ruler, notebook, blueprint, units,
  commit, result, charges. It needs no mathematics; the answer is the measurement itself.
- **Brief.** A powered cart on a straight, level rail beside the wreck drives exactly as far as it is
  told and stops. Send it to the orange marker.
- **Investigate.** Ruler: the cart's starting point to the marker centre (both pinned, on the near
  side of the rail so the cart does not hide them). The distance is a whole number of metres, 9 to 17,
  drawn after every other number so existing saves keep their layout.
- **Predict.** Enter the run distance: the notebook name (`d1`) or the number with its unit. A bare
  number is refused with the reason and costs no charge, which is the lesson about units.
- **Pass.** The cart stops on the marker (one block, so centre ± 0.5 m). Too short or too far is
  reported in metres. The rail is 24 m long; longer runs are refused.
- **Gates.** Puzzle 1 requires it.

### Puzzle 1 — Local gravity

- **Brief.** The launcher needs this planet's gravitational acceleration.
- **Investigate.** Measure the drop tower's height with the ruler. Release the test mass and time the
  fall with the stopwatch (free, repeatable).
- **Predict.** Enter `g`.
- **Test.** A second mass is released from the taller survey mast. The station shows the fall time
  predicted from the player's `g` beside the measured one.
- **Pass.** Within 2 % of the true value. The player's `g` is then written to the notebook and used by
  the later puzzles — their own measurement, not the game's number.
- **Relation.** `h = ½ g t²`.

### Puzzle 2 — Across the canyon

- **Brief.** Launch a salvage crate horizontally from the ledge so it lands on the platform on the far
  side.
- **Investigate.** Ruler: horizontal distance to the platform centre, and the drop from the ledge.
- **Predict.** Enter the launch speed.
- **Pass.** The crate lands on the platform (centre ± 1.5 m).
- **Relations.** `t = sqrt(2h/g)`, `d = v t`.

### Puzzle 3 — Over the ridge (capstone)

- **Brief.** Launch the frame beam over the ridge onto the shipyard pad. The launcher has a maximum
  speed.
- **Investigate.** Ruler: distance to the pad, distance to the ridge, height of the ridge.
- **Predict.** Enter the launch angle and the launch speed.
- **Pass.** The beam passes over the ridge without touching it and lands on the pad (5 m long, so
  centre ± 2.5 m). The tracked point is the beam's underside; the brief tells the player to leave margin.
- **Design of the numbers.** The ridge is placed so that 45° does not clear it, and the speed limit
  rules out very steep shots, leaving a window of roughly ten degrees. The player must reason about
  both constraints before computing the speed.
- **Relations.** `x = v cosθ · t`, `y = v sinθ · t − ½ g t²`; on level ground `R = v² sin 2θ / g`.
- **Reward.** The skiff's frame appears on the shipyard pad.
