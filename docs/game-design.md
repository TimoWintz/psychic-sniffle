# Game design

Working title: **First Principles**.

## Vision

You are stranded on a small voxel planet beside the wreck of your ship, which came down across an
ancient road whose builders are long gone. Their machines along it are asleep, and so is the skiff
they left on its slipway. To leave, you need a vessel three times over — first their skiff, woken
part by part, then an orbiter and a starship — and every part has to be earned by doing the physics
that the part depends on. By the time the starship leaves, the player has worked through
mechanics, electromagnetism and thermodynamics to first-year university level.

The ship's computer, **Lumen**, survived the crash with its sensors intact and its actuators gone. It
can see what you are doing and explain anything, but it cannot press a single button. Doing is the
player's job. What survived on the island is only a backup of it; the rest is across the strait (see
"The story").

Lumen is on screen as a holographic eye in the lower left corner. While it says a line it comes
forward to the bottom centre, a ring spreading from it as the line starts, and goes back after; while
a wake plays it floats above the letterbox. It points with a beam of light only
where that helps: the way to the next station for a while after it opens (and again when you stand at
a station that is done or locked), along the waking light, at places named in story lines, and at the
first station (the supply rail) to teach carrying and the ruler. At later stations, during experiments
and tests, and on solved puzzles the beam stays off so the view stays clear and the player decides what
to measure; otherwise the eye just looks about. When the place is off screen, the beam ends in an arrow at the
edge of the screen. It says short lines in a bubble above the eye when something happens (a test lands,
a station refuses an entry, a new station opens) and its eye shows a mood: pleased, concerned,
thinking. Talking to it (`T` or a click on the eye) opens the conversation where the bubble appears,
attached to the eye. None of this needs a language model; with one connected, Lumen can also turn its
eye to a place it is talking about.

Lumen also carries the story, in a few short lines and never in the way of the physics. The words are
in the chapter's file; all of them are spoken, so none holds a digit. How Lumen talks, and how every
other line in the game is written, is set out in [writing.md](writing.md).

- **Opening.** In a fresh game: who Lumen is, the main hull across the strait (its eye turns to it),
  what the chapter has to do, and the road the wreck came down across. While a line of it names a
  place, the camera turns to show it past the avatar, then turns back. The opening waits for click
  or `Enter` between lines and is not cut short: what happens meanwhile waits its turn.
- **Station briefing.** When an unsolved station (or side-machine) first opens, Lumen presents it in
  one or two short beats that wait for click or `Enter` (pointing at key parts while speaking),
  followed by clickable dialogue chips for optional questions (how the machine works, what to
  measure) and `Got it`. The console itself stays concise (one-line objective, hardware ratings,
  experiment, handbook links, blueprint, result) and includes a `Briefing` button to reopen Lumen's
  chips at any time.
- **Done.** On a pass, what the job got us and what it leads to, then the way to the next station.
- **Closing.** When the chapter's part is installed, and what woke along the way with it (waits for
  click or `Enter` between lines).
- **Remarks.** A handful of lines about places (the wreck on the road, the shore facing the strait,
  the ledge over the channel, the bridge once it stands, the ridge, the shut seal in the pass,
  the skiff on the old quay), each
  said once a session when the player comes near. Lumen makes a
  remark only when nothing is going on: no station open, no test or experiment running, the ruler
  away, nothing carried, its conversation closed, and nothing said for 40 seconds.
- **Help.** Once per station a session: after a first miss at the channel or the ridge, that the
  player's g held up at the drop tower; after two misses, if the player has not talked to it and a
  model is connected, that it can help without giving the number; when a station runs out of
  charges, that it is a good moment to check the readings.

A line can name a place; the eye holds it while the line is said, unless a station console is open or
a test or experiment is running. The dock shows the tier's goal and the
chapter's goal above the next task.

### The look of the world

Alien, but familiar. The planet has turf, rock, sand and trees, only not quite ours: the turf is teal
with drifts of olive and patches of violet lichen, the rock is slate-violet laid down in rust and pale
strata, the trees are parasols and spires of teal and violet fronds, and quartz grows in clusters that
glow. A ringed gas giant and a small moon hang in a turquoise sky. The chapter's land is an
archipelago: two islands with a narrow channel of sea between them, and other islands in the haze on
the horizon.

Two hands built what stands at the puzzle sites, and they should never be confused.

- **The way.** A road of dressed pale stone runs from the south coast to a gate on the north shore,
  and its builders' machines stand along it: a cart in a groove sunk in stone, a crawler that walks
  on six stone legs, a drop tower and a
  taller survey mast, two launchers (one in a stone jack frame on a ledge over the channel, one that
  tilts), a crane arm, a suspension bridge over the channel, and a quay with a slipway down to the
  strait, where their skiff lies. The bridge's deck hangs from cables of the same light as the road's
  quartz, and while it sleeps there is no bridge at all. Beside each machine
  stands its console, a stone with a quartz face (Lumen reads through it, the player commits through
  it), and a stone lamp. The blocks the cart carries and the jack frame takes, the casket the helm
  travels in and the helm itself are theirs too. Their stone is smooth and pale and its edges are
  worn; quartz runs in channels through the joints. A machine that is **asleep** has dark quartz and
  does not move, and neither its console nor its lamp glows. An **awake** one glows the island's
  quartz colour, console and lamp with it.
- **The castaway's marks.** What the player and Lumen add is little, and all of it is for measuring:
  the light gate, and survey orange paint on the old stone wherever a number lives: markers, timing
  posts, the tower's metre notches, the target strip, the rating plate, and the handles painted on
  what can be carried. Orange means "a number lives here" (or "this moves"), and it is always ours:
  the builders left no numbers. Apart from that, only the wreck and the scrap thrown clear of it are
  ours.

The landforms a puzzle needs (the outcrop, the channel, the ridge) are geology, not boxes: exact where
the puzzle measures them, natural everywhere else.

Every face of a block is a small picture, sixteen texels across, drawn from its material: the turf
in tufts and pale blades, the rock with fine bedding lines, the sand rippled by the wind, the soil in
clods, the fronds with gaps of shadow, the timber's grain and rings, the wreck's hull in panels. The
builders' stone is smooth, faintly clouded, with its edges worn darker, so their road and machines
read as dressed blocks. The pattern is quiet: it only lightens and darkens the block's colour, it
fades with distance rather than shimmer, and the orange paint has none, so a number's place stays the
plainest thing in view.

The sea is turquoise and clear over the sand and the shelves, and dark teal where it is deep, like the
channel. Small waves cross it, so the sun glints on it and it shows the sky at a glancing look. Where
it meets the land, a broken line of foam traces every shore and cliff, with fainter bands washing in
and out; surf breaks over the reefs just under the surface. The foam along the channel's walls also
marks where the water stops and the cliff begins.

The island is never quite still. The grass and the fronds lean in slow gusts off the sea. Motes of
light drift up from the awake lamps and consoles, and more faintly from the wild quartz. Waves throw
up spray against the cliffs; what falls into the water throws up drops beside its ring of foam; a
block set down or lifted raises a little dust. High clouds of white blocks drift over on the wind,
and flocks of flyers circle far out over the sea. None of it gets in the way of a measurement: the
clouds cast no shadows and the flocks keep well away from the island, no tufts grow at the puzzle
sites, and no motes or spray are made while a test or an experiment runs or the ruler is out.

What glows, and how much, keeps the two hands apart and shows progress at a glance. Brightest first:

1. **The builders' quartz, awake:** in the road, the machines, the consoles, the lamps, the bridge's
   cables. It has a halo, so the way reads as a thread of light across the island, and a machine
   waking is the brightest change on screen.
2. **The wild quartz** in the clusters and on the spires and shrubs: a softer glow, never as bright
   as the way.
3. **The sky's bodies:** the sun's disc, the giant, the moon.

Each chapter is played in a light of its own, which its file names (`light`): a preset that sets
the sun, the sky and the haze, so a chapter chooses a mood and the look is tuned in one place. The
light changes with the story, never with the clock, so a puzzle reads the same whenever the player
stands at it.

- **Afternoon** (1.1): a low sun in the south-west, about 35 degrees up, throws long
  shadows to the north-east. Its light is only a little warm, so the turf stays teal; the gold is in
  the sky low around the sun and in the haze towards it. The shadows are the cool blue of the sky.
- Dusk and night come with the chapters that need them: the crossing runs into the night (1.2).

Sleeping quartz is dark and has no halo. **The castaway's marks never glow:** the orange paint, the
yellow pins, the handles and the stakes are lit like the stone around them. They stay readable
because they are the most saturated colours on screen, not the brightest. Whatever is added to make
the island beautiful must leave them the first thing the eye finds at a puzzle.

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

## The story

**Premise.** The ship broke up on the way in. Its drive section is still in orbit; the main hull came
down on the mainland across the strait; the forward section, with the castaway in it, came down on the
island, across the southern end of an ancient road. A narrow channel of sea cuts the land in two from
coast to coast: the wreck's island in the south, and the north island, where the road goes on to the
skiff's quay. A bay of the strait reaches in from the north-west, so from the wreck the castaway sees
over the water to the skiff's quay and, beyond the strait, the main hull. Both stay in view from the
north island's northern shore.

**The way.** The road and its machines were built by someone who is not here any more, long ago. The
machines are asleep. Through its console, one does exactly what it is told and works nothing out:
every number still has to come from the player. When a machine's job is done right (the cart stops on
the marker, the casket lands in the strip), its quartz lights, and the light runs along the road to
the next machine, which wakes. Why the way answers work done right is never explained. Lumen notices
the pattern, wonders about it in a line or two, and is honest that it does not know. The way runs
through the whole tier: each chapter wakes another part of it, the gate on the north shore points at
the main hull, and the mainland may hold more of the builders' work in tier 2. The builders never
give a number, a relation or an answer (pillar 1), and nothing that wakes does the player's job.
A few smaller machines of theirs stand off the road and wake nothing further on (see
[Side-machines](#side-machines)): when one wakes, its light runs to the road and joins it.

**Lumen.** What survived in the forward section is a backup of Lumen, a sliver of it running on that
section's cells: sensors, no hands, and tables made for home. It does not know this planet, and it
relearns it with the player, one constant at a time, always the player's own measurement. The road
is as new to it as the planet. Its core is
in the main hull: its memory, its full instruments and the fabricator's controls. The core is silent.
Lumen looks across at it now and then, and worries about it more than it lets on. It is warm and
human, a friend beside the engineer: fond of them, glad of their hands, honestly excited when something
wakes, curious about the builders, open about what it does not know, and never mopey. Its voice is in
[writing.md](writing.md).

**Tier 1, the Skiff: cross the strait.** The skiff is the builders', and it lies asleep on its
slipway on the north shore. Nobody builds it: each chapter wakes one part of it (and more of the way),
and Lumen learns one thing about this world.

| Chapter | Part | Why the crossing needs it | What Lumen learns |
|---|---|---|---|
| 1.1 The Helm | helm | the skiff has no helm: it sits in a casket on a launcher on the wrong side of the channel | local g |
| 1.2 First Light | its lamps and power loop | the skiff's quartz is drained, the backup's cells are fading, and the crossing runs into the night | what the salvaged cells can deliver |
| 1.3 The Haul | winch, hull stones | its loose hull stones lie on the shore below the quay, and the slipway is uphill | how these grounds grip |
| 1.4 The Forge | hull stones refired | some stones are cracked, and the builders' kiln must fire them again | how the stone takes heat |
| 1.5 The Lift | drive | something has to push the skiff across | what the drive can lift and hold |
| 1.6 Skiff Trial | skiff awake | the crossing itself is the test flight | — |

The tier ends when the skiff reaches the main hull and the backup joins its core. Lumen is whole, but
the core took damage, and the drive section is still in orbit. The main hull's fabricator builds the
orbiter in tier 2, and the restored core brings the grapher, tier 2's instrument.

**Chapter 1.1: the helm.** The skiff lies on the builders' old quay on the north shore, facing the
strait and the main hull. Its helm, a long stone tiller, never reached it: it sits in a stone casket
on the launcher above the channel, as if the way was sending it there when everything stopped. The
casket is too heavy to carry, but the builders' launchers can throw it, once they can aim. The
chapter is one route from the wreck on the south coast to the quay, the southern stretch of the way,
and each station is the next machine along it. At the start all of it is asleep; only the first
console glows. Each other console lights as its machine wakes. The road crosses the channel by the
builders' bridge, which sleeps until the casket is across. On the north island the ridge runs on as a
wall, the spine, from coast to coast (a reef under the water at both ends), and the road crosses it
by one pass, which a stone seal closes until the tiller is in the landing circle.

1. The road's rail runs east from the wreck. Its first stretch is the supply rail; past a stone gate
   the same rail is the haul track, where the same cart chases the crawler to the transfer crane.
2. The drop tower and the survey mast stand at the rail's end, under the outcrop.
3. A ramp climbs north onto the outcrop, where the ledge launcher fires the casket north across the
   channel to the north island. It lands on the platform beside the tilting launcher, and a dolly
   rail takes the tiller aboard. The bridge rises out of the water, and the road runs over it.
4. The tilting launcher fires the tiller north over the ridge into the landing circle on the quay.
   Past it, the skiff lies on the quay at the head of the slipway, which runs down to the strait,
   with the main hull beyond, and the north gate stands at the quay's end.

After each pass, once the player has read the result and stepped away from the console (or after a
few seconds), the way wakes, in a short scene behind letterbox bars:

1. The machine whose job was done lights up (and, after the crawler's pass, the crane swings the
   sight off its back and sets it down at its foot; after the casket's, the bridge's pylons rise out
   of the water and its deck runs out from both banks to meet in the middle, so the light can cross;
   after the tiller's, the seal in the pass sinks, so the light can run through it).
2. A light runs up the road's quartz to the next machine; the camera and Lumen's eye follow it. Its
   head is a bright point that leaves motes of light drifting behind it along the road.
3. That machine wakes: its quartz lights and its parts move (the stone gate's leaf sinks and the
   cart rolls through, the ramp's steps rise out of the ground, the north gate's beam reaches
   across the strait), with a burst of motes where the light arrives. Its console's face and its
   lamp fade in with it, and all the way's awake quartz brightens once, as if answering. Moving
   stonework raises dust, or spray where it stands in the water.
4. The camera glides back to the player's own view, and the letterbox goes.

Only then does Lumen point on to the next job. `E` or `Esc` skips the scene. The island visibly
fills in from south to north. While the ramp sleeps the ledge cannot be reached at all: the outcrop
is sheer all round, too tall to climb even with this planet's highest jump. Until the bridge stands
the north island cannot be reached: the channel's water is too deep to wade, all the way from coast
to coast. While the seal sleeps the quay and the skiff cannot be reached either: nothing climbs the
spine, and nothing goes round it. The castaway reaches the skiff only once its helm is home.

| Station | Why, in the story | What wakes, and what changes in the world |
|---|---|---|
| The supply rail | their cart, and the one console that still glows; the cart will not run empty, so a block of theirs goes on it (and how every station works) | the marker's quartz lights and the stone gate grinds open; the cart rolls on through and becomes the haul cart |
| Catch the crawler | the builders' crawler, still walking its rounds, carries the sight: the piece the launchers aim with | the crane arm wakes and lifts the sight off; the light runs to the drop tower |
| Local gravity | the launchers work nothing out, and Lumen's backup only knows home's gravity | the tower lights to the top and the ramp onto the outcrop surfaces; the player's g is in the notebook and Lumen uses it; the sight is set on the ledge launcher |
| Across the channel | the helm, in its casket, has to cross the channel to the north island; the first real shot with the player's g | both banks light and the bridge rises across the channel; the casket stays on the platform and the tiller waits on the tilting launcher |
| Over the ridge | the tiller itself, on to the skiff | the tiller lands in the landing circle and the north gate wakes, its light pointing at the main hull; the skiff takes its helm, which settles into the socket at the stern, and the helm's quartz lights (the rest of the skiff stays dark); Lumen hears the core for the first time; the seal in the pass through the spine sinks, and the castaway can walk to the skiff (the seal only ever stood in the castaway's way, not the helm's) |

## Pillars

1. **The world is the textbook.** One voxel is one cubic metre of a real material. Distances, masses,
   resistances and heat flows in a puzzle are properties of the world the player can measure, not
   numbers printed in a problem statement.
2. **Commit, then find out.** The player works out an answer, commits it, and the world runs. Nothing is
   marked right or wrong by a teacher; the casket either lands on the platform or it does not.
3. **Understanding beats guessing.** The game is built so that calculating is the fast route and trial
   and error is the slow, expensive one.
4. **The tutor explains, the player acts.** Lumen never solves a puzzle and has no way to touch the game.
5. **Difficulty rises evenly.** All three domains advance together, tier by tier; no domain reaches
   university level while another is still at its first lesson.

## The world in SI units

- One voxel = 1 m × 1 m × 1 m of a named material.
- Every material has real properties, listed in the in-game handbook: density, specific heat, thermal
  conductivity, electrical resistivity. The handbook is the same table the simulators read.
- The planet's constants are generated per save. Gravity is not 9.81 m/s²; the third puzzle is to
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
   test mass, a block dropped beside one fired off the ledge, two shots
   at complementary angles. The player picks what they expect from a few choices (no numbers), then
   watches. Experiments are free and repeatable and give no answer away. Watching one writes its
   pages into the handbook.
3. **Predict.** Enter values, with units, in the blueprint panel. The calculator accepts expressions
   over notebook names (`(d1 + d2) / 2`, `sin(30 deg)`). Examples in the interface show how to write
   an expression, never which relation to use.
4. **Test.** Commit. The simulation runs in the world; the result panel shows what was predicted
   against what happened, with a graph.
5. **Result.** Pass: the job is done in the world (the cart reaches its marker, the casket lands on
   the platform), and the way wakes the next machine (see [The story](#the-story)); the
   last pass puts the helm in the skiff. Fail: a diagnostic says what happened in physical terms
   ("landed 3.2 m short"), never what the answer is.

### Doing it with your hands

Typing a number is not the only way to commit. Some puzzles are answered, or investigated, by moving
things in the world. Three patterns, each with the same rule: the player works it out first, then
builds it, and the exact simulator judges what was built.

| Pattern | What the player does | Where |
|---|---|---|
| Build, then compute | Work out what the world must hold, build it, then calculate the rest for what was built: blocks under a launcher whose dial only turns within a band set its height, and the speed for that height is typed. | 1.1 Across the channel (built) |
| Place to measure | Put an instrument where it measures what is wanted: a light gate clamped at a chosen notch of the drop tower, so the player designs their own measurement and collects pairs of drop and time. | 1.1 Local gravity (built) |
| Place the catcher | The setting is fixed and read off a plate; carry the target (net, stop, bin) to where the load will end up. | 1.3, the sled: place the stop where it comes to rest under friction (planned; see the curriculum) |

The verb is taught first with nothing at stake: the tutorial's cart will not run until the player
has carried a block onto it. That refusal is free, like a value of the wrong kind, and since
nothing typed was committed it does not write the tutorial's handbook page.

Placements snap to whole blocks or to fixed notches, so what is built can be counted and measured
exactly. A placement on its own is a small choice (a handful of stacks), which a few tries find by
luck, so a built answer comes with a calculated one or a small target: guessing should cost at least
as many tests as guessing a typed value, and every wrong stack also costs a walk with blocks. An
instrument that is placed (the light gate) answers nothing itself; what it measures still has to be
worked into the typed answer.

### What makes guessing a bad strategy

- **Measure how fast a guess finds the answer.** Every miss says which way it went (short or
  beyond, sooner or later), so any answer can be found by halving after each miss. How likely a
  lucky pass is, is really how many tests that halving takes: the size of the target against
  everything the station accepts, in bits. Every answer that is built in the world gets a test that
  plays such a guesser against it across many seeds: it must need well over a station's charges on
  average, and rarely pass within them. At the channel the same guesser needed 3.9 tests (31 % within three) with the
  old typed speed, 2.8 (75 %) with blocks alone at a fixed speed, and 7.6 (32 %) now that the stack is
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
  Each page reads as words with its relations set apart in boxes, and shows a short looping
  picture of the idea in a scene from Earth (a cyclist past distance posts, an apple dropped from a
  balcony, two kicks landing on one spot), strobed like the stations' experiments, with pause and
  replay. The pictures are Earth's on purpose: the castaway's home, never the island's setups, so
  they explain without answering a puzzle. Under the picture, an optional collapsible **Where this
  comes from** section walks symbolically through the derivation from earlier relations, followed by
  an optional quiz of two or three multiple-choice questions, some on ideas and some small sums with
  Earth's numbers (Earth's g may appear; the island's never comes near it). A right answer shows why;
  a wrong one sends the player back to the page. Answering every question right ticks the page, and
  Lumen says so. The quizzes change nothing in the world.
- **Launchers show the blueprint.** As a value is typed, the tilting launcher's arm swings to the
  angle and its gauge fills to the speed, against the launcher's rating. The ledge launcher's gauge
  marks the bottom of its band too and turns red outside it, and the launcher itself rides up its jack
  frame as blocks go in.
  The player's choice is visible in the world before it is committed; the path is never previewed.
- **Grapher** (tier 2 onward). Plot a notebook series; read slopes and areas.

## Side-machines

Off the road stand a few small machines of the builders that the way never needed: **side-machines**.
They are side-tracks. Nothing on the main route needs them, and they give no charges, no part
quality and nothing else the stations use. What they give is a handbook page, a line or two from
Lumen and a small change in the world.

- **No typed answer.** A side-machine is solved by moving things in the world. The world stays
  measurable as always, and counting or measuring helps, but the console never asks for a value.
- **A physics idea, a method page.** Each one rests on a physics idea the player gets a feel for by
  hand, but the page it writes is about how science is done (a rule is a guess until it predicts,
  change one thing at a time...). It never writes a physics page that a later chapter plans, so the
  chapter that teaches the idea properly still has it to teach.
- **The builders', asleep.** Each has its own console and lamp. The console answers from the start,
  like the supply rail's, but the lamp and the machine's quartz are dark, and it wakes when its job
  is done right, like every machine on the way. It doesn't wake the next machine. Instead a short
  channel of quartz runs from it and joins the road, so it was part of the way all along. Lumen
  wonders whether the builders made these to learn on, which hints that the whole way teaches without
  ever saying so.
- **Commit, then find out, for free.** What the player builds is held until they release it at the
  console, and then the world answers. Releases cost nothing and can be repeated as often as the
  player likes. A miss says which way it went and never how far.
- **Insight beats fiddling.** With no charges, pillar 3 is kept by design: the arrangements a
  machine accepts are many, few of them are right, each wrong one costs a walk with blocks, and the
  last case can't be reached from the one before by moving a single block. A test in
  `physics_core` checks this for every seed.
- **Found, not shown.** Lumen's beam never points at a side-machine, and nothing counts them. Each
  can be seen from the road, dark, and Lumen remarks on it once when the player walks near.
- **Lumen in conversation** knows that a side-machine exists and that it must not solve it for the
  player; it isn't in the snapshot the way a station is. (An answer that is an arrangement of blocks
  is a new kind of leak, which the filter doesn't guard yet.)
- **Afterwards** it stays a toy: what was built can be moved, and releases still answer, so the
  player can come back and test their rule. Its page and its wake are kept in the save.

The first is the beam, in chapter 1.1 (see the curriculum). The planned ones for the rest of tier 1
are listed there too.

## Avatar, camera, controls

Third-person view of a blocky engineer. Walk, jump, look around; `E` uses a station; `F` picks up
and puts down; `R` measures with the ruler; `N` notebook and calculator; `H` handbook; `T` (or a click
on Lumen's eye) talks to Lumen. The avatar carries one thing at a time, in its hands; there is no
inventory.

- **Stations.** While a station is open the engineer stays at its console: the movement keys do
  nothing. The camera glides to a side view of the station's puzzle, left of the panel, holding what
  the ruler measures there; it still turns and zooms around it. A test or an experiment takes the
  camera while it plays and gives it back to the station's view; leaving the station glides it home.
  A side view stands as far back as it needs to show the whole test, further than the player zooms. `E`, `Esc` or the panel's Leave button steps away. A
  station opens once its console is awake: the way wakes it with its machine.
- **Waking.** While the way wakes, the engineer waits and the camera follows the light, then glides
  back; `E` or `Esc` skips to the end. The camera watches from 28 m back, from the side where the
  fewest blocks come between it and what it follows (it looks down more steeply only if every side
  has some), and it keeps that distance: a post or pylon the light runs past hides the light for a
  moment rather than pulling the camera in to it.
- **Ruler.** `R` takes one measurement: two clicks, then the ruler stops taking clicks and its reading
  stays on screen to be noted. `R` again measures anew; `Esc` or the card's close button puts it
  away; `R` or `Esc` before the second click cancels.
- **Carrying.** What has an orange handle can be moved. `F` picks up what is in front of the
  avatar; a ghost block shows where it will go, and `F` puts it down there: on the ground, or on top
  of another block, at most one above the feet (so blocks can be stacked into steps). The builders'
  dressed blocks are real blocks: they can be stood on and measured, and each stays near where it
  belongs (the ledge's pile on the ledge, the cart's block beside the rail). Where a station reads a
  placement, putting the thing into its place is the act: a block onto the cart or into the jack frame, the
  light gate onto a notch of the drop tower (the notch is the one the pointer marks on the tower, or
  the one at hand height). The station shows what is placed there, and the save keeps it.
- **Water.** The castaway wades in the shallows. Where the water gets too deep (deeper than about a
  metre) it can walk or jump off the edge, but it falls in with a splash and comes back on the bank
  where it last stood on dry ground. A jump never carries it across deep water: out over it, the
  castaway can only get back onto the ground it stepped off. The first time, Lumen remarks on it,
  once a session.
- **Notebook, handbook, Lumen.** Always on screen. The notebook and the handbook sit in a fixed column
  at the top left under the chapter's progress, reduced to a header; `N` and `H` (or a click on the
  header) expand and reduce them. Lumen reduced is its eye; `T` expands its conversation.

The engineer is a jointed figure of blocks, animated in code. Its stride follows its speed, it
tucks on a jump and gives at the knees on landing, and it works the panel of an open station. It carries a
block in both hands, arms forward. It turns to watch a test in flight, then cheers a pass or hangs its
head at a miss. When idle, it glances where
Lumen is pointing.

## The ship and progression

The builders' skiff lies on their quay on the north shore, at the end of the chapter's route, with
the strait and the main hull beyond it. Progress shows twice over: the way wakes machine by machine
along the route, and each chapter's capstone wakes a part of the skiff on the quay; a puzzle on the
way moves what it delivers one step nearer (the helm's casket, across the channel to the tilting
launcher).

| Stage | Purpose in the story | Unlocks |
|---|---|---|
| Skiff | cross the strait to the main hull, where Lumen's core is | tier 2 stations and salvage |
| Orbiter | reach orbit, recover the drifting drive section | tier 3 stations |
| Starship | leave the system | ending |

Each tier ends with a **test flight** that needs all three domains at that level: a mass budget, a power
budget and a heat budget that must all close at once.

Chapters unlock by prerequisite, not in a fixed line: finishing a chapter opens every chapter whose
prerequisites are now met, so there are usually two or three open at once.

## Failure and tone

Failure is information. A failed test shows the physical outcome and leaves the experiment standing so
the player can look at it. Lumen is warm and brief, never condescending,
and honest when the player is wrong (see [writing.md](writing.md)).

## Out of scope for the slice

Mining or free building (only what we have marked moves, and only where a puzzle has room for it), inventory, multiplayer, sound, saving more than chapter progress, tiers 2 and 3
content, the relay server for the tutor.
