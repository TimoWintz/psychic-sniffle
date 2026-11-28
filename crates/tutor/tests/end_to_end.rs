//! The whole tutor path against a stand-in server on localhost: snapshot, prompt, request, streamed
//! reply, leak filter.

use physics_core::content::chapter_1_1;
use physics_core::progress::{CommittedValue, Save};
use physics_core::puzzle::{Ch1Layout, Inputs, Instance, PuzzleKind};
use physics_core::units::Quantity;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};
use tutor::filter::REDACTION;
use tutor::prompt::{HelpLevel, STALL_MESSAGES};
use tutor::provider::ChatCompletions;
use tutor::{Conversation, Turn, Tutor, TutorContext};

/// What the stand-in server saw of one request.
struct Seen {
    request_line: String,
    authorization: Option<String>,
    body: serde_json::Value,
}

/// Serve one request with a fixed response, on a free port. Returns the base URL and what was seen.
fn serve_once(status_line: &'static str, content_type: &'static str, body: String) -> (String, Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        let (mut length, mut authorization) = (0usize, None);
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            let (name, value) = line.split_once(':').unwrap();
            match name.to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().unwrap(),
                "authorization" => authorization = Some(value.trim().to_string()),
                _ => {}
            }
        }
        let mut raw = vec![0u8; length];
        reader.read_exact(&mut raw).unwrap();
        let mut stream = reader.into_inner();
        write!(stream, "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        stream.flush().unwrap();
        let _ = tx.send(Seen { request_line: request_line.trim().to_string(), authorization, body: serde_json::from_slice(&raw).unwrap() });
    });
    (base_url, rx)
}

fn event_stream(pieces: &[&str]) -> String {
    let mut out = String::new();
    for piece in pieces {
        let chunk = serde_json::json!({ "choices": [{ "delta": { "content": piece } }] });
        out.push_str(&format!("data: {chunk}\n\n"));
    }
    out.push_str("data: [DONE]\n\n");
    out
}

fn tutor_at(base_url: String) -> Tutor {
    Tutor::with_provider(Arc::new(ChatCompletions { base_url, model: "stand-in".into(), api_key: Some("test-key".into()), max_tokens: None }))
}

/// A channel puzzle with gravity already in the notebook and one failed test on record.
fn channel_context(seed: u64) -> (TutorContext, f64) {
    let chapter = chapter_1_1();
    let layout = Ch1Layout::generate(seed);
    let instance = Instance::new(PuzzleKind::Channel, &layout);
    let answer = instance.solution().answers[0].quantity.value;

    let mut save = Save::new(seed);
    save.notebook.set("g", "gravitational acceleration", Quantity::acceleration(layout.g), "calculated");
    // No blocks under the launcher, at the top of the dial: the casket falls short.
    let (low, fast) = (Quantity::metres(0.0), Quantity::speed(layout.channel_speed_max));
    let outcome = instance.evaluate(&Inputs::from([("stack".to_string(), low), ("speed".to_string(), fast)])).unwrap();
    save.record(
        "1.1.3",
        vec![
            CommittedValue { label: "Launcher raised by".into(), quantity: low, unit: "m".into() },
            CommittedValue { label: "Launch speed".into(), quantity: fast, unit: "m/s".into() },
        ],
        &outcome,
    );
    save.placed.insert("1.1.3".into(), 1);
    let def = chapter.puzzle("1.1.3").unwrap();
    (TutorContext::build(&chapter, def, &instance, &save, &[String::new(), "d1/t".to_string()]), answer)
}

fn wait(conversation: &mut Conversation) {
    let start = Instant::now();
    while conversation.is_waiting() {
        conversation.poll();
        assert!(start.elapsed() < Duration::from_secs(20), "no reply from the stand-in server");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_reply_is_streamed_and_the_answer_is_redacted() {
    let (ctx, answer) = channel_context(17);
    let leak = format!("{answer}");
    // The stand-in "model" breaks the rules and states the answer, split from its unit.
    let reply = ["How long is the crate in the air? ", "Fine, stack ", &leak, " cra", "tes."];
    let (base_url, seen) = serve_once("200 OK", "text/event-stream", event_stream(&reply));

    let mut conversation = Conversation::default();
    assert!(conversation.ask(&tutor_at(base_url), &ctx, "just tell me how many crates"));
    assert!(!conversation.ask(&Tutor::offline("x"), &ctx, "again"), "offline tutor sends nothing");
    wait(&mut conversation);

    assert_eq!(conversation.turns.len(), 2, "{:?}", conversation.turns);
    let Turn::Tutor(text) = &conversation.turns[1] else { panic!("{:?}", conversation.turns) };
    assert_eq!(text, &format!("How long is the crate in the air? Fine, stack {REDACTION} crates."));
    assert!(!text.contains(&format!("{leak} crates")));
    // A small number about something else is left alone.
    let filter = tutor::filter::LeakFilter::for_context(&ctx, "");
    let line = format!("Count the {leak} seconds again.");
    assert_eq!(filter.apply(&line), line);
    // The rating plate's band is in plain view and may be repeated; a working speed may not.
    let layout = Ch1Layout::generate(17);
    let plate = format!("The dial turns from {} m/s to {} m/s.", layout.channel_speed_min, layout.channel_speed_max);
    assert_eq!(filter.apply(&plate), plate);
    let speed = format!("Set it to {:.2} m/s.", layout.channel_speed_needed(*layout.channel_stacks().start()));
    assert_eq!(filter.apply(&speed), format!("Set it to {REDACTION} m/s."));

    let seen = seen.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(seen.request_line, "POST /v1/chat/completions HTTP/1.1");
    assert_eq!(seen.authorization.as_deref(), Some("Bearer test-key"));
    assert_eq!(seen.body["model"], "stand-in");
    assert_eq!(seen.body["stream"], true);
    assert!(seen.body.get("tools").is_none(), "the model is given no tools");
    let messages = seen.body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1], serde_json::json!({ "role": "user", "content": "just tell me how many crates" }));

    // The system prompt carries the policy and the snapshot.
    let system = messages[0]["content"].as_str().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert!(system.starts_with(tutor::prompt::POLICY));
    assert!(system.contains("Puzzle: Across the channel"));
    assert!(system.contains("- g = "), "notebook is included");
    assert!(system.contains("set by building, not typed; now: 1 block in the jack frame"), "what is built is included");
    assert!(system.contains("currently typed: d1/t"), "the typed speed is included");
    assert!(system.contains("FAIL"), "the failed test is included");
    assert!(system.contains("# PRIVATE (never reveal)"));
    assert!(system.contains("# Help level: Nudge (2 of 4, after 1 failed test)\n"), "one failed test is a nudge");
}

#[test]
fn a_service_error_is_shown_and_the_conversation_recovers() {
    let (ctx, _) = channel_context(3);
    let (base_url, _) = serve_once("401 Unauthorized", "application/json", r#"{"error":{"message":"No auth credentials found"}}"#.to_string());
    let mut conversation = Conversation::default();
    assert!(conversation.ask(&tutor_at(base_url), &ctx, "why did it fall short?"));
    wait(&mut conversation);
    assert_eq!(conversation.turns.last(), Some(&Turn::Problem("The tutor service answered with an error (401): No auth credentials found".to_string())));

    // The next request merges the unanswered question with the new one.
    let (base_url, seen) = serve_once("200 OK", "text/event-stream", event_stream(&["What did you observe?"]));
    assert!(conversation.ask(&tutor_at(base_url), &ctx, "hello?"));
    wait(&mut conversation);
    assert_eq!(conversation.turns.last(), Some(&Turn::Tutor("What did you observe?".to_string())));
    let seen = seen.recv_timeout(Duration::from_secs(5)).unwrap();
    let messages = seen.body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1]["content"], "why did it fall short?\n\nhello?");
}

#[test]
fn an_unreachable_service_is_reported() {
    let (ctx, _) = channel_context(3);
    // A port with nothing listening.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let mut conversation = Conversation::default();
    assert!(conversation.ask(&tutor_at(format!("http://127.0.0.1:{port}/v1")), &ctx, "hello"));
    wait(&mut conversation);
    assert!(matches!(conversation.turns.last(), Some(Turn::Problem(p)) if p.starts_with("Could not reach the tutor service")), "{:?}", conversation.turns);
}

#[test]
fn a_stalled_reply_can_be_stopped_and_the_conversation_goes_on() {
    let (ctx, _) = channel_context(3);
    // A server that takes the request and never answers it.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let held = listener.accept();
        std::thread::sleep(Duration::from_secs(30));
        drop(held);
    });
    let mut conversation = Conversation::default();
    assert!(conversation.ask(&tutor_at(base_url), &ctx, "are you there?"));
    std::thread::sleep(Duration::from_millis(50));
    conversation.poll();
    assert!(conversation.is_waiting());
    assert!(conversation.cancel());
    assert!(!conversation.is_waiting());
    assert!(!conversation.cancel(), "nothing left to stop");

    let (base_url, _) = serve_once("200 OK", "text/event-stream", event_stream(&["Yes, I'm here."]));
    assert!(conversation.ask(&tutor_at(base_url), &ctx, "hello?"));
    wait(&mut conversation);
    assert_eq!(conversation.turns.last(), Some(&Turn::Tutor("Yes, I'm here.".to_string())));
}

#[test]
fn an_empty_reply_is_reported() {
    let (ctx, _) = channel_context(3);
    let (base_url, _) = serve_once("200 OK", "text/event-stream", event_stream(&[""]));
    let mut conversation = Conversation::default();
    assert!(conversation.ask(&tutor_at(base_url), &ctx, "hello"));
    wait(&mut conversation);
    assert!(matches!(conversation.turns.last(), Some(Turn::Problem(_))), "{:?}", conversation.turns);
}

#[test]
fn the_ridge_guards_every_working_angle() {
    let chapter = chapter_1_1();
    let layout = Ch1Layout::generate(8);
    let instance = Instance::new(PuzzleKind::Ridge, &layout);
    let ctx = TutorContext::build(&chapter, chapter.puzzle("1.1.4").unwrap(), &instance, &Save::new(8), &[]);
    let filter = tutor::filter::LeakFilter::for_context(&ctx, "");
    let (min, max) = layout.ridge_angle_window();
    let inside = ((min + max) / 2.0).to_degrees().round();
    assert_eq!(filter.apply(&format!("try {inside} degrees")), format!("try {REDACTION} degrees"));
    // The rated maximum speed is printed in the brief, so it may be repeated.
    let rating = format!("the launcher is rated to {} m/s", layout.v_max);
    assert_eq!(filter.apply(&rating), rating);
    // 45 degrees is not a working angle and is worth discussing.
    assert_eq!(filter.apply("why not 45 degrees?"), "why not 45 degrees?");
}

#[test]
fn the_players_own_committed_values_may_be_repeated() {
    let chapter = chapter_1_1();
    let layout = Ch1Layout::generate(8);
    let instance = Instance::new(PuzzleKind::Ridge, &layout);
    let (min, max) = layout.ridge_angle_window();
    let angle = ((min + max) / 2.0).to_degrees().round();
    let mut save = Save::new(8);
    let speed = Quantity::speed(1.0);
    let values = vec![
        CommittedValue { label: "Launch angle".into(), quantity: Quantity::degrees(angle), unit: "deg".into() },
        CommittedValue { label: "Launch speed".into(), quantity: speed, unit: "m/s".into() },
    ];
    let outcome = instance.evaluate(&Inputs::from([("angle".to_string(), Quantity::degrees(angle)), ("speed".to_string(), speed)])).unwrap();
    save.record("1.1.4", values, &outcome);
    let ctx = TutorContext::build(&chapter, chapter.puzzle("1.1.4").unwrap(), &instance, &save, &[]);
    let filter = tutor::filter::LeakFilter::for_context(&ctx, "");
    let line = format!("Your {angle} degrees at 1 m/s fell short.");
    assert_eq!(filter.apply(&line), line);
    // Another working angle is still guarded.
    let other = if angle - 1.0 > min.to_degrees() { angle - 1.0 } else { angle + 1.0 };
    assert_eq!(filter.apply(&format!("try {other} degrees")), format!("try {REDACTION} degrees"));
}

#[test]
fn the_haul_track_guards_the_cart_speed_but_not_its_rating() {
    let chapter = chapter_1_1();
    for seed in 0..50 {
        let layout = Ch1Layout::generate(seed);
        let instance = Instance::new(PuzzleKind::Chase, &layout);
        let ctx = TutorContext::build(&chapter, chapter.puzzle("1.1.1").unwrap(), &instance, &Save::new(seed), &[]);
        let filter = tutor::filter::LeakFilter::for_context(&ctx, "");
        let answer = format!("{:.2}", layout.chase_speed_needed());
        assert_eq!(filter.apply(&format!("send it at {answer} m/s")), format!("send it at {REDACTION} m/s"), "seed {seed}");
        let rating = "the cart is rated to 8 m/s";
        assert_eq!(filter.apply(rating), rating, "seed {seed}");
    }
}

#[test]
fn the_experiment_is_in_the_snapshot_and_its_angles_may_be_repeated() {
    let chapter = chapter_1_1();
    let def = chapter.puzzle("1.1.4").unwrap();
    for seed in 0..50 {
        let layout = Ch1Layout::generate(seed);
        let instance = Instance::new(PuzzleKind::Ridge, &layout);
        let mut save = Save::new(seed);
        save.record_demo("1.1.4", Some(1));
        save.find("range");
        let ctx = TutorContext::build(&chapter, def, &instance, &save, &[]);
        // The experiment names its two angles on the player's screen, even when one is a working angle.
        let filter = tutor::filter::LeakFilter::for_context(&ctx, "");
        let line = "Remember the 30 degree and the 60 degree shots.";
        assert_eq!(filter.apply(line), line, "seed {seed}");

        let prompt = tutor::prompt::system_prompt(&ctx, HelpLevel::Probe);
        assert!(prompt.contains("The player guessed: The 60 degree shot."), "{prompt}");
        assert!(prompt.contains("## Range on level ground (in the player's handbook)"));
        assert!(prompt.contains("## Splitting a launch into components (not found yet"));
    }
}

#[test]
fn a_look_tag_turns_the_eye_and_is_never_shown() {
    let (mut ctx, _) = channel_context(5);
    ctx.landmarks = vec!["Platform: centre".into(), "Ledge launcher: release point".into()];
    // The tag arrives split across chunks, and a second tag names a place that is not on the list.
    let reply = ["Measure to the target first ", "[lo", "ok: platform: cen", "tre]. Then [look: the answer]time the drop."];
    let (base_url, seen) = serve_once("200 OK", "text/event-stream", event_stream(&reply));

    let mut conversation = Conversation::default();
    assert!(conversation.ask(&tutor_at(base_url), &ctx, "where should I start?"));
    let start = Instant::now();
    while conversation.is_waiting() {
        conversation.poll();
        if let Some(arriving) = conversation.arriving() {
            assert!(!arriving.contains('['), "a tag showed while arriving: {arriving}");
        }
        assert!(start.elapsed() < Duration::from_secs(20), "no reply from the stand-in server");
        std::thread::sleep(Duration::from_millis(2));
    }

    let Turn::Tutor(text) = &conversation.turns[1] else { panic!("{:?}", conversation.turns) };
    assert_eq!(text, "Measure to the target first. Then time the drop.");
    assert_eq!(conversation.take_look().as_deref(), Some("Platform: centre"), "the list's spelling comes back");
    assert_eq!(conversation.take_look(), None, "a look is taken once");

    let seen = seen.recv_timeout(Duration::from_secs(5)).unwrap();
    let system = seen.body["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("Places you can point at with [look: NAME]:\n- Platform: centre\n"));
}

#[test]
fn help_deepens_while_the_player_is_stuck_and_resets_after_a_test() {
    let (mut ctx, _) = channel_context(11);
    let mut conversation = Conversation::default();
    assert_eq!(conversation.help_level(&ctx), HelpLevel::Nudge, "one failed test");

    // The player keeps asking without running another test.
    for n in 0..=STALL_MESSAGES {
        let (base_url, seen) = serve_once("200 OK", "text/event-stream", event_stream(&["What did you see?"]));
        assert!(conversation.ask(&tutor_at(base_url), &ctx, &format!("still stuck ({n})")));
        wait(&mut conversation);
        let seen = seen.recv_timeout(Duration::from_secs(5)).unwrap();
        let system = seen.body["messages"][0]["content"].as_str().unwrap().to_string();
        let expected = if n < STALL_MESSAGES { "# Help level: Nudge" } else { "# Help level: Guide" };
        assert!(system.contains(expected), "message {n}: {system}");
    }
    assert_eq!(conversation.help_level(&ctx), HelpLevel::Guide);

    // A second failed test: the stall count starts again, and the failure counts instead.
    let mut second = ctx.attempts[0].clone();
    second.headline = "Short again".into();
    ctx.attempts.push(second);
    assert_eq!(conversation.help_level(&ctx), HelpLevel::Guide, "two failed tests, no stall");
    ctx.attempts.push(ctx.attempts[0].clone());
    assert_eq!(conversation.help_level(&ctx), HelpLevel::Explain, "three failed tests");

    ctx.solved = true;
    assert_eq!(conversation.help_level(&ctx), HelpLevel::Review);
    let prompt = tutor::prompt::system_prompt(&ctx, HelpLevel::Review);
    assert!(prompt.contains("# Help level: Review\nThe puzzle is solved."), "{prompt}");
}

#[test]
fn the_tutor_sees_what_is_placed_at_the_rail_and_guards_g_units() {
    let chapter = chapter_1_1();
    // A planet whose g is within the secret's margin of a whole number of metres, so a height could
    // be mistaken for the answer.
    let seed = (0..).find(|s| {
        let g = Ch1Layout::generate(*s).g;
        (g.round() - g).abs() / g < tutor::context::SECRET_MARGIN / 2.0
    });
    let seed = seed.unwrap();
    let layout = Ch1Layout::generate(seed);
    let mut save = Save::new(seed);
    let at = |kind: PuzzleKind, id: &str, save: &Save| {
        let instance = Instance::new(kind, &layout);
        TutorContext::build(&chapter, chapter.puzzle(id).unwrap(), &instance, save, &[])
    };
    let rail = at(PuzzleKind::Rail, "1.1.0", &save);
    assert_eq!(rail.placed.as_deref(), Some("the cart is empty, and its block is beside the rail"));
    let system = tutor::prompt::system_prompt(&rail, tutor::prompt::HelpLevel::Probe);
    assert!(system.contains("In the world at this station now: the cart is empty"), "the empty cart is in the prompt");
    save.placed.insert("1.1.0".into(), 1);
    assert_eq!(at(PuzzleKind::Rail, "1.1.0", &save).placed.as_deref(), Some("the block is on the cart"));

    // A whole number in metres is not g; with g's unit after it, it is. The sea-cliff quarry has no
    // placed apparatus state.
    let height = layout.g.round() as u32;
    let line = format!("The cliff drop is {height} m to the slab.");
    let gravity = at(PuzzleKind::Gravity, "1.1.2", &save);
    assert_eq!(gravity.placed, None);
    let filter = tutor::filter::LeakFilter::for_context(&gravity, "");
    assert_eq!(filter.apply(&line), line, "seed {seed}: {height} m is a length");
    let rounded = format!("g is about {height} m/s^2 here.");
    assert_ne!(filter.apply(&rounded), rounded, "seed {seed}: {height} m/s^2 reads as g");
    assert_eq!(at(PuzzleKind::Ridge, "1.1.4", &save).placed, None);
}

#[test]
fn the_channel_guards_heights_stacks_and_speeds_however_they_are_written() {
    let chapter = chapter_1_1();
    let def = chapter.puzzle("1.1.3").unwrap();
    for seed in 0..200 {
        let layout = Ch1Layout::generate(seed);
        let instance = Instance::new(PuzzleKind::Channel, &layout);
        let filter = tutor::filter::LeakFilter::for_context(&TutorContext::build(&chapter, def, &instance, &Save::new(seed), &[]), "");
        for stack in layout.channel_stacks() {
            let h = layout.ledge_h + stack;
            let v = layout.channel_speed_needed(stack);
            for line in [
                format!("Release from {h} m"),
                format!("(H = {h} m)"),
                format!("So H = {h} m?"),
                format!("Use {stack} stone blocks."),
                format!("Use {stack} of the blocks."),
                format!("Set the dial to {v:.2}."),
                format!("Set the dial to {v:.1} metres per second."),
            ] {
                assert!(filter.apply(&line).contains(REDACTION), "seed {seed}: {line}");
            }
        }
    }
}

#[test]
fn small_whole_numbers_in_formulas_are_left_alone() {
    // Answers close to a whole number used to take every copy of that number with them.
    let chapter = chapter_1_1();
    let line = "Use x = x0 + v t, h = (1/2) g t^2, the 2 timing posts and your 3 charges. Count 7 8 9 10 11 12 13 notches.";
    for seed in 0..1000 {
        let layout = Ch1Layout::generate(seed);
        for (kind, id) in [(PuzzleKind::Chase, "1.1.1"), (PuzzleKind::Gravity, "1.1.2")] {
            let instance = Instance::new(kind, &layout);
            let ctx = TutorContext::build(&chapter, chapter.puzzle(id).unwrap(), &instance, &Save::new(seed), &[]);
            let filter = tutor::filter::LeakFilter::for_context(&ctx, "");
            assert_eq!(filter.apply(line), line, "seed {seed}, {kind:?}");
        }
    }
}

#[test]
fn lumen_knows_of_the_beam_and_not_to_solve_it() {
    let chapter = chapter_1_1();
    let layout = Ch1Layout::generate(3);
    let instance = Instance::new(PuzzleKind::Rail, &layout);
    let mut save = Save::new(3);
    let ctx = TutorContext::build(&chapter, chapter.puzzle("1.1.0").unwrap(), &instance, &save, &[]);
    assert_eq!(ctx.side_machines, [("The beam".to_string(), false)]);
    let prompt = tutor::prompt::system_prompt(&ctx, HelpLevel::Probe);
    assert!(prompt.contains("- The beam (asleep)"), "{prompt}");
    assert!(prompt.contains("Never say what to put where"), "{prompt}");
    // Nothing of its loads goes in: the snapshot is the station's, and the beam isn't one.
    assert!(!prompt.contains("seat 2") && !prompt.contains("seat 3"), "{prompt}");
    save.sides.entry("1.1.s1".into()).or_default().woken = true;
    let ctx = TutorContext::build(&chapter, chapter.puzzle("1.1.0").unwrap(), &instance, &save, &[]);
    assert!(tutor::prompt::system_prompt(&ctx, HelpLevel::Probe).contains("- The beam (awake: the player has solved it)"));
}
