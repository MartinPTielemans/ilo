//! What ilo tells a task it stopped over its helpers.

use super::*;

/// What ilo tells a task it stopped counts as a sentence does, one helper or several, with the
/// limit it's held to, and a helper's own helper is said as that and not as the task's count.
#[test]
fn a_stopped_task_is_told_its_limit_as_a_sentence() {
    let several = MOST.max(2);
    assert!(told(Limit::Count, 1).contains("more than 1 helper."));
    assert!(told(Limit::Count, several).contains(&format!("more than {several} helpers.")));
    for most in [1, several] {
        let told = told(Limit::Count, most);
        assert!(told.contains(&format!("at most {most} for each message")));
        assert!(
            told.starts_with(&format!("{PRODUCT} stopped that turn.")),
            "{told}"
        );
    }
    let deep = told(Limit::Depth, several);
    assert!(
        deep.contains("A helper started a helper of its own."),
        "{deep}"
    );
    assert!(!deep.contains("You started"), "{deep}");
    // A task ilo stopped is told so, and the kind it may start by its name.
    let kind = told(Limit::Kind, several);
    assert!(
        kind.starts_with(&format!("{PRODUCT} stopped this task.")),
        "{kind}"
    );
    assert!(
        deep.starts_with(&format!("{PRODUCT} stopped this task.")),
        "{deep}"
    );
    assert!(kind.contains(KIND), "{kind}");
}
