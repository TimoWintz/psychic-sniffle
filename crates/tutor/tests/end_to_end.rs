//! The whole tutor path against a stand-in server on localhost: snapshot, prompt, request, streamed
//! reply, leak filter.

use physics_core::content::chapter_1_1;
use physics_core::progress::{CommittedValue, Save};
use physics_core::puzzle::{Ch1Layout, Instance, Inputs, PuzzleKind};
use physics_core::units::Quantity;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};
use tutor::filter::REDACTION;
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
        write!(
            stream,
            "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        stream.flush().unwrap();
        let _ = tx.send(Seen {
            request_line: request_line.trim().to_string(),
            authorization,
            body: serde_json::from_slice(&raw).unwrap(),
        });
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
    Tutor::with_provider(Arc::new(ChatCompletions {
        base_url,
        model: "stand-in".into(),
        api_key: Some("test-key".into()),
        max_tokens: None,
    }))
}

/// A canyon puzzle with gravity already in the notebook and one failed test on record.
fn canyon_context(seed: u64) -> (TutorContext, f64) {
    let chapter = chapter_1_1();
    let layout = Ch1Layout::generate(seed);
    let instance = Instance::new(PuzzleKind::Canyon, &layout);
    let answer = instance.solution().answers[0].quantity.value;

    let mut save = Save::new(seed);
    save.notebook.set("g", "gravitational acceleration", Quantity::acceleration(layout.g), "calculated");
    let slow = Quantity::speed(answer * 0.5);
    let outcome = instance.evaluate(&Inputs::from([("speed".to_string(), slow)])).unwrap();
    save.record(
        "1.1.2",
        vec![CommittedValue { label: "Launch speed".into(), quantity: slow, unit: "m/s".into() }],
        &outcome,
    );
    let def = chapter.puzzle("1.1.2").unwrap();
    (TutorContext::build(&chapter, def, &instance, &save, &["d1/t".to_string()]), answer)
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
    let (ctx, answer) = canyon_context(17);
    let leak = format!("{answer:.2}");
    // The stand-in "model" breaks the rules and states the answer, split across chunks.
    let (head, tail) = leak.split_at(2);
    let reply = ["How long is the crate in the air? ", "Fine, the speed is ", head, tail, " m/s."];
    let (base_url, seen) = serve_once("200 OK", "text/event-stream", event_stream(&reply));

    let mut conversation = Conversation::default();
    assert!(conversation.ask(&tutor_at(base_url), &ctx, "just tell me the speed"));
    assert!(!conversation.ask(&Tutor::offline("x"), &ctx, "again"), "offline tutor sends nothing");
    wait(&mut conversation);

    assert_eq!(conversation.turns.len(), 2, "{:?}", conversation.turns);
    let Turn::Tutor(text) = &conversation.turns[1] else { panic!("{:?}", conversation.turns) };
    assert_eq!(text, &format!("How long is the crate in the air? Fine, the speed is {REDACTION} m/s."));
    assert!(!text.contains(&leak));

    let seen = seen.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(seen.request_line, "POST /v1/chat/completions HTTP/1.1");
    assert_eq!(seen.authorization.as_deref(), Some("Bearer test-key"));
    assert_eq!(seen.body["model"], "stand-in");
    assert_eq!(seen.body["stream"], true);
    assert!(seen.body.get("tools").is_none(), "the model is given no tools");
    let messages = seen.body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1], serde_json::json!({ "role": "user", "content": "just tell me the speed" }));

    // The system prompt carries the policy and the snapshot.
    let system = messages[0]["content"].as_str().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert!(system.starts_with(tutor::prompt::POLICY));
    assert!(system.contains("Puzzle: Across the canyon"));
    assert!(system.contains("- g = "), "notebook is included");
    assert!(system.contains("currently typed: d1/t"), "blueprint draft is included");
    assert!(system.contains("FAIL"), "the failed test is included");
    assert!(system.contains("# PRIVATE — never reveal"));
}

#[test]
fn a_service_error_is_shown_and_the_conversation_recovers() {
    let (ctx, _) = canyon_context(3);
    let (base_url, _) = serve_once(
        "401 Unauthorized",
        "application/json",
        r#"{"error":{"message":"No auth credentials found"}}"#.to_string(),
    );
    let mut conversation = Conversation::default();
    assert!(conversation.ask(&tutor_at(base_url), &ctx, "why did it fall short?"));
    wait(&mut conversation);
    assert_eq!(
        conversation.turns.last(),
        Some(&Turn::Problem(
            "The tutor service answered with an error (401): No auth credentials found".to_string()
        ))
    );

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
    let (ctx, _) = canyon_context(3);
    // A port with nothing listening.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let mut conversation = Conversation::default();
    assert!(conversation.ask(&tutor_at(format!("http://127.0.0.1:{port}/v1")), &ctx, "hello"));
    wait(&mut conversation);
    assert!(
        matches!(conversation.turns.last(), Some(Turn::Problem(p)) if p.starts_with("Could not reach the tutor service")),
        "{:?}",
        conversation.turns
    );
}

#[test]
fn the_ridge_guards_every_working_angle() {
    let chapter = chapter_1_1();
    let layout = Ch1Layout::generate(8);
    let instance = Instance::new(PuzzleKind::Ridge, &layout);
    let ctx = TutorContext::build(&chapter, chapter.puzzle("1.1.3").unwrap(), &instance, &Save::new(8), &[]);
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
