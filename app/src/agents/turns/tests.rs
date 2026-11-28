//! A conversation's every order: what the user says, the model streams and the tools answer.

use std::time::{Duration, Instant};

use serde_json::json;

use super::*;
use crate::agents::model::Stop;

fn turns(context: u32) -> Turns {
    Turns::new(
        Caps::plain(context),
        "Work in the browser.".into(),
        Vec::new(),
        None,
        100,
    )
}

fn text(text: &str) -> Vec<Part> {
    vec![Part::Text(text.into())]
}

fn call(id: &str, name: &str) -> Delta {
    Delta::Call {
        id: id.into(),
        name: name.into(),
        args: json!({ "tab": 1 }),
    }
}

/// The request of the one Model step among `steps`, and its turn.
fn model(steps: &[Step]) -> (u32, &Request) {
    let models: Vec<_> = steps
        .iter()
        .filter_map(|step| match step {
            Step::Model { turn, request } => Some((*turn, request)),
            _ => None,
        })
        .collect();
    assert_eq!(models.len(), 1, "one model call in {steps:?}");
    models[0]
}

fn outs(steps: &[Step]) -> Vec<Out> {
    steps
        .iter()
        .filter_map(|step| match step {
            Step::Out(out) => Some(out.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_turn_writes_as_it_streams_and_says_it_whole_at_its_end() {
    let mut turns = turns(100_000);
    let now = Instant::now();
    let steps = turns.say(text("Find a flight"), false);
    assert_eq!(steps[0], Step::Out(Out::Writing(String::new())));
    let (turn, request) = model(&steps);
    assert_eq!(request.items, [Item::User(text("Find a flight"))]);
    assert_eq!(
        outs(&turns.delta(turn, Delta::Text("Two".into()), now)),
        [Out::Writing("Two".into())]
    );
    assert_eq!(
        outs(&turns.delta(turn, Delta::Text(" shows".into()), now)),
        [Out::Writing("Two shows".into())]
    );
    assert_eq!(
        turns.delta(turn, Delta::Ended(Stop::Done), now),
        [Step::Out(Out::Said("Two shows".into())), Step::Ended(None)]
    );
    assert!(!turns.busy());
    assert_eq!(turns.items.last(), Some(&Item::Said("Two shows".into())));
}

#[test]
fn parallel_calls_run_as_they_come_and_the_model_goes_on_with_every_result_in_call_order() {
    let mut turns = turns(100_000);
    let now = Instant::now();
    let (turn, _) = model(&turns.say(text("Compare both"), false));
    turns.delta(turn, Delta::Text("Looking.".into()), now);
    let steps = turns.delta(turn, call("a", "read"), now);
    assert_eq!(
        steps,
        [
            Step::Out(Out::Said("Looking.".into())),
            Step::Out(Out::Calling),
            Step::Run {
                id: "a".into(),
                name: "read".into(),
                args: json!({ "tab": 1 })
            },
        ]
    );
    let steps = turns.delta(turn, call("b", "read"), now);
    assert!(matches!(steps.last(), Some(Step::Run { id, .. }) if id == "b"));
    // The second result in before the model's end: it waits for the first and for the end.
    assert!(turns.result("b", text("page b"), false).is_empty());
    assert!(turns.delta(turn, Delta::Ended(Stop::Calls), now).is_empty());
    let steps = turns.result("a", text("page a"), false);
    let (next, request) = model(&steps);
    assert_eq!(next, turn + 1);
    let tail: Vec<_> = request.items[request.items.len() - 2..].to_vec();
    assert_eq!(
        tail,
        [
            Item::Result {
                id: "a".into(),
                parts: text("page a"),
                error: false
            },
            Item::Result {
                id: "b".into(),
                parts: text("page b"),
                error: false
            },
        ]
    );
}

#[test]
fn a_message_sent_now_goes_in_with_the_next_results() {
    let mut turns = turns(100_000);
    let now = Instant::now();
    let (turn, _) = model(&turns.say(text("Book it"), false));
    turns.delta(turn, call("a", "open"), now);
    assert!(turns.say(text("Window seat"), true).is_empty());
    turns.delta(turn, Delta::Ended(Stop::Calls), now);
    let steps = turns.result("a", text("opened"), false);
    let (_, request) = model(&steps);
    assert_eq!(request.items.last(), Some(&Item::User(text("Window seat"))));
}

#[test]
fn a_model_that_takes_no_steering_hears_a_message_sent_now_after_its_turn() {
    let mut turns = Turns::new(
        Caps {
            steer: Steer::QueueOnly,
            ..Caps::plain(100_000)
        },
        String::new(),
        Vec::new(),
        None,
        100,
    );
    let now = Instant::now();
    let (turn, _) = model(&turns.say(text("Book it"), false));
    turns.delta(turn, call("a", "open"), now);
    turns.say(text("Window seat"), true);
    turns.delta(turn, Delta::Ended(Stop::Calls), now);
    let steps = turns.result("a", text("opened"), false);
    let (turn, request) = model(&steps);
    assert!(matches!(request.items.last(), Some(Item::Result { .. })));
    let steps = turns.delta(turn, Delta::Ended(Stop::Done), now);
    let (_, request) = model(&steps);
    assert_eq!(request.items.last(), Some(&Item::User(text("Window seat"))));
}

#[test]
fn a_queued_message_follows_the_turn_with_no_end_between() {
    let mut turns = turns(100_000);
    let now = Instant::now();
    let (turn, _) = model(&turns.say(text("Find a flight"), false));
    turns.delta(turn, Delta::Text("Found one.".into()), now);
    assert!(turns.say(text("And a hotel"), false).is_empty());
    assert_eq!(turns.queued().count(), 1);
    let steps = turns.delta(turn, Delta::Ended(Stop::Done), now);
    assert!(!steps.iter().any(|step| matches!(step, Step::Ended(_))));
    assert_eq!(steps[0], Step::Out(Out::Said("Found one.".into())));
    let (next, request) = model(&steps);
    assert_eq!(next, turn + 1);
    assert_eq!(
        request.items[request.items.len() - 2..],
        [
            Item::Said("Found one.".into()),
            Item::User(text("And a hotel"))
        ]
    );
}

#[test]
fn stopping_answers_each_call_out_cancels_the_turn_and_keeps_the_queue() {
    let mut turns = turns(100_000);
    let now = Instant::now();
    let (turn, _) = model(&turns.say(text("Book it"), false));
    turns.delta(turn, call("a", "open"), now);
    turns.say(text("Later"), false);
    turns.say(text("Aisle"), true);
    assert_eq!(turns.stop(), [Step::Cancel(turn), Step::Ended(None)]);
    assert!(!turns.busy());
    assert_eq!(
        turns.items.last(),
        Some(&Item::Result {
            id: "a".into(),
            parts: text(STOPPED),
            error: true
        })
    );
    // The stopped turn's stream and its call's answer aren't heard.
    assert!(
        turns
            .delta(turn, Delta::Text("late".into()), now)
            .is_empty()
    );
    assert!(turns.result("a", text("opened"), false).is_empty());
    // What waited goes first, the steer it was given ahead of the queue.
    let queued: Vec<_> = turns.queued().cloned().collect();
    assert_eq!(queued, [text("Aisle"), text("Later")]);
    let steps = turns.say(text("Go on"), false);
    let (_, request) = model(&steps);
    assert_eq!(
        request.items[request.items.len() - 3..],
        [
            Item::User(text("Aisle")),
            Item::User(text("Later")),
            Item::User(text("Go on"))
        ]
    );
}

#[test]
fn a_failure_ends_the_turn_with_why_and_answers_the_calls_out() {
    let mut turns = turns(100_000);
    let now = Instant::now();
    let (turn, _) = model(&turns.say(text("Book it"), false));
    turns.delta(turn, call("a", "open"), now);
    assert_eq!(
        turns.delta(turn, Delta::Failed(Failure::Credit), now),
        [Step::Ended(Some(Failure::Credit))]
    );
    assert!(!turns.busy());
    assert!(matches!(
        turns.items.last(),
        Some(Item::Result { error: true, .. })
    ));
}

#[test]
fn thinking_is_a_thought_that_took_until_what_followed_it() {
    let mut turns = turns(100_000);
    let start = Instant::now();
    let (turn, _) = model(&turns.say(text("Plan it"), false));
    turns.delta(turn, Delta::Thinking("Weighing ".into()), start);
    turns.delta(
        turn,
        Delta::Thinking("both.".into()),
        start + Duration::from_secs(1),
    );
    let steps = turns.delta(
        turn,
        Delta::Text("Done.".into()),
        start + Duration::from_secs(3),
    );
    assert_eq!(
        outs(&steps),
        [
            Out::Thought {
                text: "Weighing both.".into(),
                took: Duration::from_secs(3)
            },
            Out::Writing("Done.".into())
        ]
    );
}

#[test]
fn signed_reasoning_stays_in_place_before_what_followed_it() {
    let mut turns = turns(100_000);
    let now = Instant::now();
    let (turn, _) = model(&turns.say(text("Plan it"), false));
    let signed = crate::agents::model::Signed {
        maker: "fake".into(),
        text: "plan".into(),
        signature: "sig".into(),
    };
    turns.delta(turn, Delta::Signed(signed.clone()), now);
    turns.delta(turn, Delta::Text("Opening.".into()), now);
    turns.delta(turn, call("a", "open"), now);
    assert_eq!(
        turns.items[1..3],
        [Item::Thought(signed), Item::Said("Opening.".into())]
    );
}

#[test]
fn older_results_give_way_to_a_line_naming_them_once_the_request_nears_the_context() {
    // 1000 tokens of context, 800 of room, less 100 out: 700, so two 1200-character results fit
    // and a third doesn't.
    let mut turns = turns(1000);
    let now = Instant::now();
    let page = "x".repeat(1200);
    let (mut turn, _) = model(&turns.say(text("Read three"), false));
    for (id, name) in [("a", "read"), ("b", "find"), ("c", "read")] {
        turns.delta(turn, call(id, name), now);
        turns.delta(turn, Delta::Ended(Stop::Calls), now);
        let steps = turns.result(id, text(&page), false);
        turn = model(&steps).0;
    }
    let results: Vec<_> = turns
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Result { parts, .. } => Some(parts.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        results,
        [
            text("[read result from step 1 dropped to save room]"),
            text(&page),
            text(&page)
        ]
    );
}

#[test]
fn a_conversation_still_too_long_is_summed_up_before_the_users_latest_turn() {
    let mut turns = turns(1000);
    let now = Instant::now();
    let long = "y".repeat(3200);
    let (turn, _) = model(&turns.say(text(&long), false));
    turns.delta(turn, Delta::Text("Read it.".into()), now);
    turns.delta(turn, Delta::Ended(Stop::Done), now);
    // Nothing to drop but the first turn itself: it's summed up, and nothing of that is shown.
    let steps = turns.say(text("Now book"), false);
    assert!(outs(&steps).is_empty());
    let (summing, request) = model(&steps);
    assert_eq!(request.items.len(), 3);
    assert_eq!(request.items[2], Item::User(text(SUMMARIZE)));
    assert!(outs(&turns.delta(summing, Delta::Text("Asked to read.".into()), now)).is_empty());
    let steps = turns.delta(summing, Delta::Ended(Stop::Done), now);
    let (turn, request) = model(&steps);
    assert_eq!(turn, summing + 1);
    assert_eq!(
        request.items,
        [
            Item::User(text(&format!("{SUMMARY}\n\nAsked to read."))),
            Item::User(text("Now book"))
        ]
    );
}
