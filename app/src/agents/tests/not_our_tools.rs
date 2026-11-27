//! A Claude thread whose run starts with a tool that isn't the browser's, or calls one
//! (runner.rs): ilo ends the run before its first answer, says so in one line of its own (AP17),
//! and the thread is Interrupted.

use super::kept::Run;
use super::*;
use crate::agents::runner::{Cli, Out};
use crate::agents::task::ilo::IloKind;
use crate::agents::task::{State, Word};
use crate::product::PRODUCT;

/// How many of ilo's lines about the run's tools the thread's conversation has.
fn refused_lines(run: &Run, agent: &str, cx: &mut TestAppContext) -> usize {
    (run.ilo_lines(agent, cx).into_iter())
        .filter(|kind| *kind == IloKind::NotOurTools)
        .count()
}

#[gpui::test]
fn a_run_ilo_refuses_ends_the_turn_with_ilos_line(cx: &mut TestAppContext) {
    let run = Run::new("init-refused", cx);
    let agent = run.start(Cli::Claude, cx);
    let name = (run.agents).read_with(cx, |agents, _| agents.task(&agent).unwrap().name.clone());
    run.said(
        &agent,
        Out::Refused("it started with the tool \"Bash\"".into()),
        cx,
    );
    let state = run.state(&agent, cx);
    let State::Failed { why, .. } = &state else {
        panic!("{state:?}")
    };
    assert_eq!(
        why.to_string(),
        format!("{PRODUCT} stopped {name}: it started with tools {PRODUCT} doesn't allow.")
    );
    assert!(!why.contains("Bash"), "{why}");
    assert_eq!(Word::of(&state, false), Word::Interrupted);
    assert_eq!(refused_lines(&run, &agent, cx), 1);
    // The run's end that follows changes nothing: no second line, and no other words.
    run.said(&agent, Out::Gone(String::new()), cx);
    assert_eq!(run.state(&agent, cx), state);
    assert_eq!(refused_lines(&run, &agent, cx), 1);
}
