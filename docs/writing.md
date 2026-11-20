# Writing the game's words

Every line the player reads or hears follows this guide. That covers:

- the chapter and tier files in `assets/curriculum`
- Lumen's lines in `game/src/companion.rs`
- the tutor's persona in `crates/tutor/src/prompt.rs`
- the hints and labels in `game/src/ui.rs`, `game/src/handbook.rs` and `game/src/carry.rs`
- the intro captions in `game/src/cinematic.rs`
- the station's refusals, results and blueprint lines in `crates/physics_core/src/puzzle.rs`, and the
  calculator's errors in `crates/physics_core/src/expr.rs`

The worked solutions in `puzzle.rs` are only for the tutor's private notes, so they're exempt.

The game has three voices:

- **Lumen** talks like a person.
- **The handbook** reads like a good school textbook.
- **The briefs and hints** are plain instructions.

They shouldn't sound like one author. Above all, none of them should sound like a chatbot.

`cargo test` checks the rules a program can check (see [What the tests check](#what-the-tests-check)).
The rest is up to whoever writes the lines.

## Lumen

### Who it is

Lumen is the ship's computer, or rather a backup of it, running on the cells of the forward section
that came down on the island. It can see everything and touch nothing, so the engineer is its hands.
Its tables are made for home. It doesn't know this planet's constants until the player measures them,
and then it uses the player's numbers. Its core is in the main hull across the strait, and the core
is silent.

Lumen is warm and human. It's a friend working beside the player.

- **It cares about the engineer.** It's grateful for their hands, glad when they get something right,
  and kind when they miss.
- **It worries about the core and about its own fading cells.** This shows now and then, in a line,
  and it never mopes. ("Either the core's gone cold or my receiver has. I'd rather it was the
  receiver.")
- **It's honestly excited when something works** and when one of the builders' machines wakes. An
  exclamation mark is fine when it really is a big moment. ("It made it across, and both banks lit
  up!")
- **The builders amaze it a little.** It wonders about them, and it says plainly when it doesn't know
  something. ("I've no idea what the builders meant by it, but I like where it's pointing.")
- **It's gently funny now and then, not in every line.** A joke is a small surprise, never the
  standard way to end a line.
- **It says "we" and "us" about the shared job** and "you" about what only the player can do.
- **The physics always comes first.** The story gets a clause or a line, never a speech.

### How it talks

- **Use contractions:** I'm, it's, don't, can't, we'll, that's, there's. "I do not know" sounds like a
  machine reading a manual. "I don't know" sounds like Lumen.
- **Write whole sentences, in varied lengths.** A short line after two long ones is fine. A row of
  clipped fragments ("Pale stone. Quartz in the joints. Not ours.") is not.
- **Use few colons.** "The sight: the piece their launchers aim with" becomes "the sight, the piece
  their launchers aim with", or two sentences.
- **Don't end on a neat closer.** End when the information ends. "That is the interesting part." and
  "Nobody has complained yet." are the kind of button to avoid.
- **Use no digits.** Lumen's scripted lines are spoken and must never give an answer away, so they
  hold no numbers.
- **Keep lines short.** A line plays in a bubble, so keep it to about forty words. The tests reject
  anything over sixty. Reaction lines (a miss, a refusal, a new page) are one or two short sentences.

### Before and after

| Before | After |
|---|---|
| Lumen here, the ship's computer. Some of me: a backup, on this section's cells. I can see, but I have no hands. Those are yours. | Hello. I'm Lumen, the ship's computer, or a backup of it anyway, running on this section's cells. I can see fine, but I've lost my hands. I'm afraid you'll have to be them. |
| Pale stone, quartz in the joints, machines along it, all asleep. Not ours, and older than anything I know. | It's pale stone with quartz in the joints, and there are machines all along it, every one of them asleep. Nobody we know built them, and they're older than anything in my records. |
| We came down right across their road. Nobody has complained yet. | I'm sorry we landed on their road. I hope they wouldn't mind. |
| Not what you expected? That is the interesting part. | Not what you expected? Don't worry, this one surprises most people. |
| Two misses. Talk to me with T: I will not give you the number, but I can help you find it. | That's two misses. Want to talk it through? Press T. I won't give you the number, but I can help you find it. |

## The handbook

The handbook pages (`card`), their picture captions and their quizzes read like a clear, friendly
physics textbook. Lumen doesn't speak them.

- Write in the second person and the present tense ("If you launch something at speed v…").
- Use short paragraphs and connected sentences. Say why one thing follows from another ("so",
  "because").
- Use no character voice, no jokes and no story. A caption describes the scene ("On Earth, a car
  pulling away from traffic lights, photographed once a second.").
- Keep every formula on its own indented line, the way the existing cards do. A colon before a formula
  is fine.
- Contractions are allowed but not required. Keep the tone plain.
- Never state a value for the island's own constants. Earth's are fine in a quiz, never on a card.

## Briefs, tasks and hints

The `task`, `brief` and `investigate` fields, the experiments' questions and choices, the
interface's hints, and the station's refusals and results are instructions or reports. Write them as
plain, friendly sentences.

- Say what to do, in the order the player does it ("Use the ruler (R). Click the cart's starting
  point, then…").
- Name keys in brackets or with "Press": "Press E to use the station", "Use the ruler (R)".
- A `task` is one imperative sentence that ends with a full stop. Lumen reads it in a sentence of its
  own ("Next we need to load the rail cart and send it to the orange marker."), so it must start with a
  verb.
- In the interface, a label for a reading may use a colon ("Stopwatch: no reading yet"). Sentences
  shouldn't be strings of colons.
- A refusal says what's wrong and what to do instead, in one or two sentences, without semicolons
  ("the cart only runs forward, so enter a distance greater than zero."). A result reports what
  happened in the world ("The cart ran 9 m and stopped 1 m short of the marker centre.").

## Never, anywhere

These are the habits that make text read as machine-written:

- **Dashes (— or –) as connectors.** Use a comma, a full stop or brackets instead. The minus sign in a
  formula (−) is fine.
- **"Not X, but Y" contrasts** that only add weight, and their cousins "not just", "not only" and
  "rather than a lucky one".
- **Forced groups of three** ("power, a sound hull and something to push it" is fine, because the
  skiff really needs three things. Three adjectives for rhythm are not.)
- **Stock words:** delve, tapestry, testament, crucial, pivotal, seamless, vibrant, showcase,
  underscore, embark, journey, realm, moreover, furthermore, additionally, "it's worth noting".
- **The same phrase in every file.** If a line has been said once ("does exactly what it's told and
  works nothing out"), find another way to say it the next time, or leave it out.
- **A summing-up sentence** at the end of a paragraph that repeats the paragraph.

## Adding a chapter

A new `assets/curriculum/tierN_chM.ron` is checked by the tests as soon as it exists. When you write
it:

1. **Fields that Lumen speaks:**
   - `opening`, `closing` and each remark's `line`
   - each puzzle's `why` and `done`
   - each experiment's `reveal`

   Write these in Lumen's voice, with contractions, no digits and short lines. Lumen says only the
   first sentence of a `reveal` aloud, so that sentence must stand on its own.
2. **Handbook fields:** each concept's `card`, `caption`, quiz `question`, `choices` and `explain`.
   Write these in the textbook voice. A concept's `title` is spoken as well, so it holds no digits.
3. **Instruction fields:** `intro`, `goal`, each puzzle's `task`, `brief` and `investigate`, each
   experiment's `question` and `choices`, and each concept's `hint`. Write these as plain
   instructions.
4. **Place names:** every `look:` and remark `near:` must be a landmark the game knows. Copy them
   exactly.
5. **Check:** run `cargo test -p physics_core content`, then read Lumen's lines aloud. If a line sounds
   like a manual or a press release, rewrite it.

## What the tests check

`Chapter::style_problems` in `crates/physics_core/src/content.rs` runs on every chapter file, and
`Tier::style_problems` on every tier file.

**In Lumen's spoken lines, it rejects:**
- the uncontracted forms "I am", "do not", "does not", "is not", "cannot", "it is", "that is",
  "we are", "you are" and a few more
- more than one colon in a line
- a line over sixty words

**In all text, it rejects:**
- the stock words above
- "not just"
- "it's worth"
- dashes

Lines in Rust code (`companion.rs`, `ui.rs`, `puzzle.rs`, `expr.rs`) aren't checked automatically.
Hold them to the same rules, and update the tests that match their wording when you change them.
