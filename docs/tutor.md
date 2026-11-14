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
  stations, the wreck, the shipyard). It turns Lumen's on-screen eye toward that place and does nothing
  else. The tag is removed before the text is shown (also while it streams), and a name not on the list
  is ignored, so the tag cannot carry text past the leak filter.

## What the tutor sees

One snapshot per message, built at the moment the player sends it:

- chapter and puzzle title, the brief, the concepts being taught;
- what the puzzle asks for (each input, with its dimension);
- the player's notebook (every reading taken so far);
- the values currently typed in the blueprint, if any;
- every attempt so far: what was committed and what happened;
- the names of the places Lumen can point at;
- the worked solution and the answer values — marked private.

The tutor is given the solution because a tutor that does not know the answer cannot tell a sign error
from a wrong method. Keeping the answer from the player is handled below.

## Teaching policy (the system prompt)

1. Find out what the player has tried before explaining anything.
2. Escalate only as far as needed: the concept, then the relation between quantities, then how to set
   the problem up. Stop there.
3. Never state a final numeric answer for a value the puzzle asks for, and never do the final
   substitution of the player's numbers. Confirm or deny the player's method; do not confirm a number.
4. When an attempt failed, start from the observed outcome ("it landed short — what does that say about
   the speed?").
5. Point at the player's own measurements by notebook name.
6. Be brief: a few sentences, one question at a time.
7. Say plainly when the player is wrong, and why.
8. Stay on physics and this game.
9. You cannot act in the game. If asked to, say so and tell the player how to do it themselves.
10. Everything marked private is for judging the player's reasoning, never for quoting.
11. To draw the player's eye to a place, write `[look: NAME]` with a listed name; at most one per reply.

## Answer-leak filter

Instructions alone will not hold against a determined player, so the reply passes through a filter
before it is shown:

- every number in the reply is compared with the private answer values (in SI and in the units a
  player would naturally use, such as degrees for an angle);
- a number within 2 % of an answer is replaced with `[yours to work out]`;
- where many answers work (the ridge), the whole working range is guarded when followed by its unit;
- numbers the player already has (notebook readings, values in the brief, numbers they typed in the
  chat) are never redacted;
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
