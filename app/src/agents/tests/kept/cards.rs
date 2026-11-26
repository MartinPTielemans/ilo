//! A kept task's card across a quit (#4090): what its Done writes to the card's folder, the
//! picture only one an agent could have had, nothing with the switch off, and each way the card
//! goes. On the stand-in assistant and a profile folder of the tests' own.

use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame, RgbaImage};

use super::past::opens_a_page;
use super::*;
use crate::agents::kept::cards::{self, CardId};

/// A made-up picture, 2 by 2 pixels.
pub(super) fn picture() -> Arc<RenderImage> {
    let pixels = RgbaImage::from_pixel(2, 2, image::Rgba([40, 80, 120, 255]));
    Arc::new(RenderImage::new(vec![Frame::new(pixels)]))
}

/// The task's agent opens a page: the tab.
pub(super) fn last_page(run: &Run, agent: &str, cx: &mut TestAppContext) -> u64 {
    let space = opens_a_page(&run.agents, agent, cx);
    let tabs = run.browser.tabs.borrow();
    tabs.iter()
        .rev()
        .find(|tab| tab.1 == space)
        .expect("its page")
        .0
}

/// The task's conversation took a picture of `tab`: `picture`, or none if it couldn't be given.
pub(super) fn pictured(
    run: &Run,
    agent: &str,
    tab: u64,
    picture: Option<Arc<RenderImage>>,
    cx: &mut TestAppContext,
) {
    (run.agents).update(cx, |agents, cx| {
        agents.card_pictured(agent, tab, picture, cx)
    });
}

fn root(run: &Run) -> PathBuf {
    run.profile.join(cards::FOLDER)
}

fn card_id(run: &Run, agent: &str, cx: &mut TestAppContext) -> CardId {
    (run.agents).read_with(cx, |agents, _| {
        (agents.tasks.kept.of(agent).expect("nothing kept"))
            .card
            .clone()
    })
}

/// What the task's card folder holds, once what was asked of it is done.
fn card(run: &Run, agent: &str, cx: &mut TestAppContext) -> cards::Card {
    cx.run_until_parked();
    cards::read(&root(run), &card_id(run, agent, cx))
}

fn size(card: &cards::Card) -> Option<(u32, u32)> {
    (card.picture.as_ref()).map(|picture| (picture.width, picture.height))
}

/// A task started, at work on a page of its own.
fn at_work(run: &Run, at: usize, cx: &mut TestAppContext) -> (String, u64) {
    let agent = run.start(Cli::Claude, cx);
    run.ran(at);
    let tab = last_page(run, &agent, cx);
    (agent, tab)
}

/// A kept task's Done writes its card: the line its card shows and the last picture its
/// conversation took of its last page, under an id of the card's own that the file of kept tasks
/// holds. The line is in the card's folder alone: no other file under the profile has it.
#[gpui::test]
fn a_done_kept_task_keeps_its_line_and_its_last_pages_picture(cx: &mut TestAppContext) {
    let run = Run::new("card", cx);
    let (agent, tab) = at_work(&run, 1, cx);
    pictured(&run, &agent, tab, Some(picture()), cx);
    run.finish(&agent, cx);
    let card = card(&run, &agent, cx);
    assert_eq!(card.line.as_deref(), Some("Two seats at 9:05, car 4."));
    assert_eq!(size(&card), Some((2, 2)));
    let id = card_id(&run, &agent, cx);
    let index = std::fs::read_to_string(run.profile.join(kept::FOLDER).join("index.json")).unwrap();
    assert!(index.contains(id.as_str()));
    for (path, text) in run.files() {
        if !path.starts_with(root(&run)) {
            assert!(!text.contains("car 4"), "{}", path.display());
        }
    }
}

/// A conversation shown with no picture taken keeps the line alone.
#[gpui::test]
fn a_task_with_no_picture_keeps_its_line_alone(cx: &mut TestAppContext) {
    let run = Run::new("card-line", cx);
    let (agent, _) = at_work(&run, 1, cx);
    run.finish(&agent, cx);
    let card = card(&run, &agent, cx);
    assert_eq!(card.line.as_deref(), Some("Two seats at 9:05, car 4."));
    assert_eq!(card.picture, None);
}

/// The last picture is the card's: one after it that couldn't have been given to an agent leaves
/// the card none, never the one before; and a picture of another page is never a card's.
#[gpui::test]
fn a_card_keeps_no_older_picture_nor_another_pages(cx: &mut TestAppContext) {
    let run = Run::new("card-later", cx);
    let (agent, tab) = at_work(&run, 1, cx);
    pictured(&run, &agent, tab, Some(picture()), cx);
    pictured(&run, &agent, tab, None, cx);
    run.finish(&agent, cx);
    assert_eq!(card(&run, &agent, cx).picture, None);

    let (other, tab) = at_work(&run, 2, cx);
    pictured(&run, &other, tab + 1000, Some(picture()), cx);
    run.finish(&other, cx);
    let card = card(&run, &other, cx);
    assert_eq!(card.picture, None);
    assert!(card.line.is_some());
}

/// With Settings' switch off nothing of a card is written, though the task was kept before.
#[gpui::test]
fn with_the_switch_off_nothing_of_a_card_is_written(cx: &mut TestAppContext) {
    let run = Run::new("card-off", cx);
    let (agent, tab) = at_work(&run, 1, cx);
    (run.agents).update(cx, |agents, cx| agents.set_keeps_tasks(false, cx));
    pictured(&run, &agent, tab, Some(picture()), cx);
    run.finish(&agent, cx);
    cx.run_until_parked();
    assert!(!root(&run).exists());
}

/// A card goes with its task: deleted from the Archive, removed from Settings, with Remove All
/// and with its profile. Each of its files goes, and the file of kept tasks no longer has its id.
#[gpui::test]
fn a_card_goes_with_its_task_and_remove_all_leaves_none(cx: &mut TestAppContext) {
    let run = Run::new("card-out", cx);
    run.browser.profiles.borrow_mut().push((7, "Profile 2"));
    let mut made = Vec::new();
    for at in 1..=3 {
        let (agent, tab) = at_work(&run, at, cx);
        pictured(&run, &agent, tab, Some(picture()), cx);
        run.finish(&agent, cx);
        made.push((agent.clone(), card_id(&run, &agent, cx)));
    }
    let elsewhere = (run.agents)
        .update(cx, |agents, cx| {
            let asked = "Find two seats to Porto for Sunday";
            agents.start_task(Cli::Claude, asked, None, 7, Tier::default(), cx)
        })
        .expect("started");
    run.ran(4);
    run.finish(&elsewhere, cx);
    let elsewhere = card_id(&run, &elsewhere, cx);
    cx.run_until_parked();
    let card_folder = |id: &CardId| root(&run).join(id.as_str());
    assert!(made.iter().all(|(_, id)| card_folder(id).exists()));
    assert!(card_folder(&elsewhere).exists());

    let index = || std::fs::read_to_string(run.profile.join(kept::FOLDER).join("index.json"));
    let (deleted, removed, left) = (&made[0], &made[1], &made[2]);
    (run.agents).update(cx, |agents, cx| {
        _ = agents.task_action(&deleted.0, TaskAction::Dismiss, cx);
        _ = agents.task_action(&deleted.0, TaskAction::Delete, cx);
    });
    (run.agents).update(cx, |agents, cx| agents.remove_kept_task(&removed.0, cx));
    run.purged(cx);
    assert!(card_folder(&removed.1).symlink_metadata().is_err());
    assert!(!index().unwrap().contains(removed.1.as_str()));

    let both = vec!["Default".to_string(), "Profile 2".to_string()];
    (run.agents).update(cx, |agents, cx| agents.profiles_are(both, cx));
    let one = vec!["Default".to_string()];
    (run.agents).update(cx, |agents, cx| agents.profiles_are(one, cx));
    run.purged(cx);
    assert!(!card_folder(&elsewhere).exists());
    assert!(card_folder(&left.1).exists());

    (run.agents).update(cx, |agents, cx| agents.remove_kept_tasks(cx));
    run.purged(cx);
    assert!(!root(&run).exists());
}

/// At the Done, a picture is written only if its page could still be kept: one taken before the
/// user held the page keeps none, and a later Done after the user held it takes out the one an
/// earlier Done wrote.
#[gpui::test]
fn a_page_held_since_its_picture_keeps_none_at_the_done(cx: &mut TestAppContext) {
    let run = Run::new("card-held", cx);
    let hold = |agent: &str, tab: u64, cx: &mut TestAppContext| {
        (run.agents).update(cx, |agents, cx| agents.take_over(agent, tab, cx));
        (run.agents).update(cx, |agents, cx| agents.hand_back(tab, cx));
    };
    let (agent, tab) = at_work(&run, 1, cx);
    pictured(&run, &agent, tab, Some(picture()), cx);
    hold(&agent, tab, cx);
    run.finish(&agent, cx);
    let held = card(&run, &agent, cx);
    assert!(held.line.is_some());
    assert_eq!(held.picture, None);

    // On a run of its own, as the stand-in browser gives every opened page the same tab.
    let run = Run::new("card-held-again", cx);
    let (other, tab) = at_work(&run, 1, cx);
    pictured(&run, &other, tab, Some(picture()), cx);
    run.finish(&other, cx);
    assert_eq!(size(&card(&run, &other, cx)), Some((2, 2)));
    (run.agents).update(cx, |agents, cx| agents.reply(&other, "And for Sunday?", cx));
    assert_eq!(run.state(&other, cx), State::Working);
    (run.agents).update(cx, |agents, cx| agents.take_over(&other, tab, cx));
    (run.agents).update(cx, |agents, cx| agents.hand_back(tab, cx));
    run.finish(&other, cx);
    let after = card(&run, &other, cx);
    assert!(after.line.is_some());
    assert_eq!(after.picture, None);
}

/// Deleted from the Archive, a thread has no row, and nothing of its card is written then or
/// while it has none; its card stays while ⌘Z can bring it back, and goes when the thread goes
/// for good, in this run or at the next launch.
#[gpui::test]
fn a_deleted_threads_card_goes_when_the_thread_goes_for_good(cx: &mut TestAppContext) {
    let run = Run::new("card-deleted", cx);
    let mut made = Vec::new();
    for at in 1..=2 {
        let (agent, tab) = at_work(&run, at, cx);
        pictured(&run, &agent, tab, Some(picture()), cx);
        run.finish(&agent, cx);
        let id = card_id(&run, &agent, cx);
        cx.run_until_parked();
        // So a write at the Delete shows.
        cards::write(&root(&run), &id, Some("As it was."), &cards::Picture::Leave);
        (run.agents).update(cx, |agents, cx| {
            _ = agents.task_action(&agent, TaskAction::Dismiss, cx);
            _ = agents.task_action(&agent, TaskAction::Delete, cx);
        });
        let card = cards::read(&root(&run), &id);
        cx.run_until_parked();
        assert_eq!(card.line.as_deref(), Some("As it was."));
        assert_eq!(size(&card), Some((2, 2)));
        made.push((agent, id));
    }
    let (dropped, quit) = (&made[0], &made[1]);
    (run.agents).update(cx, |agents, cx| agents.drop_task(&dropped.0, cx));
    run.purged(cx);
    assert!(!root(&run).join(dropped.1.as_str()).exists());
    assert!(root(&run).join(quit.1.as_str()).exists());

    (run.agents).update(cx, |agents, cx| agents.end_tasks(true, cx));
    let next = Run::again(run.profile.clone(), run.program.clone(), cx);
    next.purged(cx);
    assert!(!root(&next).join(quit.1.as_str()).exists());
}

/// The next launch takes out a card whose task is gone and keeps the one a kept task holds; with
/// no kept task at all, it takes out every card.
#[gpui::test]
fn the_launch_takes_a_card_no_kept_task_holds(cx: &mut TestAppContext) {
    let run = Run::new("card-sweep", cx);
    let (agent, tab) = at_work(&run, 1, cx);
    pictured(&run, &agent, tab, Some(picture()), cx);
    run.finish(&agent, cx);
    let kept = card_id(&run, &agent, cx);
    let stray = CardId::new();
    cx.run_until_parked();
    cards::write(
        &root(&run),
        &stray,
        Some("Left behind."),
        &cards::Picture::Remove,
    );
    (run.agents).update(cx, |agents, cx| agents.end_tasks(true, cx));

    let next = Run::again(run.profile.clone(), run.program.clone(), cx);
    cx.run_until_parked();
    assert!(!root(&next).join(stray.as_str()).exists());
    assert!(root(&next).join(kept.as_str()).exists());

    std::fs::remove_file(next.profile.join(kept::FOLDER).join("index.json")).unwrap();
    let last = Run::again(next.profile.clone(), next.program.clone(), cx);
    cx.run_until_parked();
    assert!(!root(&last).exists());
}

/// A picture is one an agent could have had only while no one holds its page and since the last
/// new document after anyone did: not while the user holds it, nor after they let go until a load
/// of a new document, which a same-document navigation isn't; not once an agent typed into it
/// what may not be shown, until the same, by the tool or a step of run_steps, though what may be
/// shown, a choice or a key keeps it; and never at a fenced site.
#[gpui::test]
fn a_picture_is_kept_only_while_an_agent_could_have_had_it(cx: &mut TestAppContext) {
    let run = Run::new("card-rules", cx);
    let (agent, tab) = at_work(&run, 1, cx);
    let keeps = |cx: &mut TestAppContext| {
        (run.agents).read_with(cx, |agents, cx| agents.keeps_picture(tab, cx))
    };
    let state = |url: &str, loading: bool, cx: &mut TestAppContext| {
        (run.agents).update(cx, |agents, _| agents.tab_state(tab, url, loading))
    };
    let new_document = |url: &str, cx: &mut TestAppContext| {
        state(url, true, cx);
        (run.agents).update(cx, |agents, _| agents.tab_loaded(tab));
        state(url, false, cx);
    };
    state("https://www.example.com/timetable", false, cx);
    assert!(keeps(cx));

    (run.agents).update(cx, |agents, cx| agents.take_over(&agent, tab, cx));
    assert!(!keeps(cx));
    (run.agents).update(cx, |agents, cx| agents.hand_back(tab, cx));
    assert!(!keeps(cx), "letting go is no new document");
    state("https://www.example.com/timetable#seats", false, cx);
    assert!(!keeps(cx), "a same-document navigation");
    new_document("https://www.example.com/seats", cx);
    assert!(keeps(cx));

    // The core answers each act with the page's tree: a field anything typed in may show, and
    // one whose name says it's a secret.
    let answered = |cx: &mut TestAppContext| {
        let tree = json!({ "nodes": [
            { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Seats" },
              "childIds": ["2", "3"] },
            { "nodeId": "2", "role": { "value": "textbox" }, "name": { "value": "Coach" },
              "backendDOMNodeId": 31 },
            { "nodeId": "3", "role": { "value": "textbox" }, "name": { "value": "Passcode" },
              "backendDOMNodeId": 41 },
        ]});
        let request = run.browser.pages.borrow().last().unwrap().1;
        (run.agents).update(cx, |agents, cx| {
            agents.page_done(request, tree.to_string(), None, cx)
        });
        cx.run_until_parked();
    };
    let act = |tool: &str, arguments: Value, cx: &mut TestAppContext| {
        _ = call(&run.agents, &agent, tool, arguments, cx);
        cx.run_until_parked();
        answered(cx);
    };
    // The page it opened.
    answered(cx);
    act("snapshot", json!({ "tab": tab }), cx);
    act("type", json!({ "tab": tab, "ref": "e1", "text": "4" }), cx);
    act(
        "select",
        json!({ "tab": tab, "ref": "e1", "option": "4" }),
        cx,
    );
    act("press_key", json!({ "tab": tab, "key": "Tab" }), cx);
    assert!(keeps(cx), "what may be shown, a choice and a key keep it");
    act(
        "type",
        json!({ "tab": tab, "ref": "e2", "text": "4242" }),
        cx,
    );
    assert!(!keeps(cx));
    new_document("https://www.example.com/paid", cx);
    assert!(keeps(cx));
    // A step of run_steps marks as the tool alone does.
    act("snapshot", json!({ "tab": tab }), cx);
    let typed = json!({ "action": "type", "ref": "e2", "text": "4242" });
    act("run_steps", json!({ "tab": tab, "steps": [typed] }), cx);
    assert!(!keeps(cx));
    new_document("https://www.example.com/thanks", cx);
    assert!(keeps(cx));

    (run.agents).update(cx, |agents, cx| _ = agents.fence("example.com", cx));
    assert!(!keeps(cx));
}
