# The tutor (Lumen)

Lumen is a language model that explains physics to the player. It can read the state of the current
puzzle. It cannot change anything in the game.

## Read-only, by construction

The guarantee does not rest on instructions to the model.

- The `tutor` crate does not depend on the game engine. It cannot name, let alone call, anything that
  changes game state. The compiler enforces this.
- The game hands the tutor a **snapshot**: a plain data value copied out of the game. The tutor returns
  text. There is no channel in the other direction other than that text being shown in the chat panel.
- The model is given no tools.
- The game never parses the tutor's reply for commands. The one thing read from it is a gaze: a
  `[look: NAME]` tag, where NAME must be one of the places listed in the snapshot (survey pins,
  stations, the wreck, the skiff). It turns Lumen's on-screen eye toward that place and does nothing
  else. The tag is removed before the text is shown (also while it streams), and a name not on the list
  is ignored, so the tag cannot carry text past the leak filter.

## What the tutor sees

One snapshot per message, built at the moment the player sends it:

- the story: what the tier's ship is for, the chapter's goal, and why this station matters (the line
  Lumen says when it opens), with a note in the policy on who Lumen is (a backup with home's tables,
  its core across the strait) and that the physics comes first;
- chapter and puzzle title, the brief, the concepts being taught and which of their pages the player
  has found;
- the station's experiment: its question and choices, the player's guess before the first run, and
  what happened (or that it has not been watched);
- what the puzzle asks for (each input, with its dimension);
- the player's notebook (every reading taken so far);
- the values currently typed in the blueprint, if any;
- every attempt so far: what was committed and what happened;
- the names of the places Lumen can point at;
- the current help level (see below);
- the worked solution and the answer values — marked private.

The tutor is given the solution because a tutor that does not know the answer cannot tell a sign error
from a wrong method. Keeping the answer from the player is handled below.

## Teaching policy (the system prompt)

Lumen teaches by asking. The policy (`tutor::prompt::POLICY`) also gives the model Lumen's character
and way of talking, kept in step with [writing.md](writing.md). Its teaching rules:

1. Teach by asking: find out what the player thinks, has tried or predicts before explaining; when
   they are partly right, say which part and ask about the gap. Do not lecture.
2. Explain only as far as the current help level allows (below), even when the player asks for more;
   then give the one question that moves them on.
3. Never state a final numeric answer for a value the puzzle asks for, and never do the final
   substitution of the player's numbers. Confirm or deny the player's method; do not confirm a number.
4. When an attempt failed, start from the observed outcome ("it landed short — what does that say about
   the speed?").
5. Point at the player's own measurements by notebook name.
6. Be brief: a few sentences, one question at a time, unless the help level allows more.
7. Say plainly when the player is wrong. At the lower levels, ask a question that lets them find out
   why; from Guide on, say why.
8. Stay on physics and this game.
9. You cannot act in the game. If asked to, say so and tell the player how to do it themselves.
10. Everything marked private is for judging the player's reasoning, never for quoting.
11. To draw the player's eye to a place, write `[look: NAME]` with a listed name; at most one per reply.
12. Build on what the player saw in the station's experiment and name handbook pages by their titles; if
    a page the puzzle needs is not found yet, send the player to the experiment.

### Help levels

Each request carries a help level (`tutor::prompt::HelpLevel`) that says how far Lumen may go. Its
score rises by one per failed test on the puzzle, and by one more for every 4 messages the player
sends without running a test (at most two steps that way, so the top level needs at least one failed test).
A new test resets the message count.

| Level   | Score             | Lumen may…                                                                                |
|---------|-------------------|-------------------------------------------------------------------------------------------|
| Probe   | 0                 | only ask; point to the experiment or a handbook page; no formulas the player hasn't written |
| Nudge   | 1                 | start from the last test's outcome, name the idea and which readings matter                |
| Guide   | 2                 | state the relation as a formula, check the setup step by step, name the first wrong step   |
| Explain | 3 or more         | walk through the whole method in symbols and say exactly where the player went wrong; describe a bad reading in words only; leave the substitution and arithmetic to the player |
| Review  | puzzle solved     | discuss the method freely and in depth, still without the numbers                          |

The message count lives in the conversation and is not saved, so it starts again when the game
restarts. Failed tests are saved and always count.

## Answer-leak filter

Instructions alone will not hold against a determined player, so the reply passes through a filter
before it is shown:

- every number in the reply is compared with the private answer values (in SI and in the units a
  player would naturally use, such as degrees for an angle);
- a number within 2 % of an answer is replaced with `[yours to work out]`. For an answer that is
  measured rather than counted (a speed, g, an angle), a number written without decimals counts only
  with the answer's unit after it, so "about 12 m/s" is caught but the 2 in "(1/2) g t^2" is not;
- where many answers work (the ridge, the channel), the whole working range is guarded when followed
  by its unit. A unit counts as a whole word ("8 m" and "(8 m)", not "8 m/s"), and a count of blocks
  may have a couple of words before its noun ("5 stone blocks");
- numbers the player already has (notebook readings, values in the brief, the experiment's question
  and choices, numbers they typed in the chat) are never redacted;
- while a reply is still streaming, a number that is still arriving is held back until it is complete.

The filter is deliberately simple and errs on the side of redacting.

## Providers

The game is not tied to a model vendor. `settings.toml` beside the executable selects one:

```toml
[tutor]
base_url    = "https://openrouter.ai/api/v1"   # or any service using the same request format
model       = "<model name as the service spells it>"
api_key_env = "OPENROUTER_API_KEY"             # name of the environment variable holding the key
```

- The key is read from the named environment variable and never written to disk by the game.
- `api_key_env` may be omitted for a locally run model that needs no key.
- No provider is built in. Without a `[tutor]` section the chat panel reads "Lumen is offline" and the
  handbook's concept cards remain available.

The slice ships one adapter, for the "chat completions" request format with streamed replies. That
format is accepted by OpenRouter, OpenAI and most local model servers. Other connection types — a
vendor's own format, or signing in with an existing subscription instead of a key — are further
implementations of the same small interface (`Provider`: take a conversation, stream back text).

## Distribution

A shipped game cannot carry a key. The route is a small relay server that holds the key, applies rate
limits per player, and forwards requests; the game would point `base_url` at it. Not built in the slice.

## Privacy

The snapshot contains only game state and what the player types into the chat. Nothing else leaves the
machine. The settings screen will say which service receives it.
