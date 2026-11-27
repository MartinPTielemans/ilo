//! The user's tabs picked under a thread's conversation: the reply field's add menu, its chips,
//! and what the reply carries, on a host that records what the shell asks of the core.

use super::*;
use crate::tab_picks::{ALL_TABS, YOUR_TABS};

/// Task-1's conversation shown in a window whose Space lists today's 10 and 11, each on a page
/// of the web, and the view that draws it.
fn replying(
    cx: &mut TestAppContext,
) -> (
    Entity<Shell>,
    Rc<Recorder>,
    Entity<super::super::TalkView>,
    &mut VisualTestContext,
) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx);
        for (tab, title) in [(10, "Ten"), (11, "Eleven")] {
            let url = format!("https://site{tab}.test/").into();
            shell.set_page(
                tab,
                Page {
                    url,
                    ..Page::default()
                },
                cx,
            );
            shell.set_title(tab, title.into(), cx);
        }
    });
    cx.run_until_parked();
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    host.take();
    (shell, host, view, cx)
}

/// The add menu's rows as a click on the reply field's plus shows them; None where it shows no
/// menu.
fn add_menu(
    view: &Entity<super::super::TalkView>,
    cx: &mut VisualTestContext,
) -> Option<Vec<MenuEntry>> {
    let shown: Rc<RefCell<Option<Vec<MenuEntry>>>> = Rc::default();
    let into = shown.clone();
    cx.update(|_, cx| {
        cx.set_global(crate::menu::EntriesMenu(Rc::new(
            move |items, _, _, _, _, _| *into.borrow_mut() = Some(items),
        )))
    });
    cx.run_until_parked();
    let plus = view.read_with(cx, |view, _| view.plus.drawn())?;
    cx.simulate_click(plus.center(), Modifiers::none());
    cx.run_until_parked();
    shown.borrow_mut().take()
}

/// The user's click on the row titled `title` of the add menu, opened for it.
fn click_tab(view: &Entity<super::super::TalkView>, title: &str, cx: &mut VisualTestContext) {
    let rows = add_menu(view, cx).expect("the plus showed no menu");
    let row = (rows.iter().find(|row| row.title.to_string() == title)).expect("no such row");
    let stays = row.stays.clone().expect("a tab's row closes the menu");
    cx.update(|_, cx| stays(cx));
    cx.run_until_parked();
}

fn chips(view: &Entity<super::super::TalkView>, cx: &mut VisualTestContext) -> Vec<u64> {
    view.read_with(cx, |view, _| view.picks.tabs())
}

fn replies(host: &Recorder) -> Vec<String> {
    let calls = host.take().into_iter();
    calls.filter(|call| call.contains("Reply")).collect()
}

/// Under a thread the plus opens the add menu on the row for the user's tabs, which lists
/// those of the Space in view the thread may be handed, asked once as the menu opens. A click
/// picks one.
#[gpui::test]
fn the_reply_fields_add_menu_lists_the_users_tabs(cx: &mut TestAppContext) {
    let (_, host, view, cx) = replying(cx);
    assert!(host.hand_asks.borrow().is_empty(), "asked before the menu");
    let rows = add_menu(&view, cx).expect("the plus showed no menu");
    let titles: Vec<_> = rows.iter().map(|row| row.title.to_string()).collect();
    assert_eq!(titles, [YOUR_TABS, ALL_TABS, "Ten", "Eleven"]);
    assert_eq!(
        *host.hand_asks.borrow(),
        [(Some("task-1".to_string()), 1, 2)]
    );
    click_tab(&view, "Eleven", cx);
    assert_eq!(chips(&view, cx), [11]);
    let rows = add_menu(&view, cx).unwrap();
    assert!(
        rows[3].checked && !rows[2].checked && !rows[1].checked,
        "the pick isn't checked"
    );
}

/// A tab the app would not hand this thread is left out of the menu.
#[gpui::test]
fn a_tab_the_thread_cant_be_handed_isnt_in_the_reply_fields_menu(cx: &mut TestAppContext) {
    let (_, host, view, cx) = replying(cx);
    host.not_to_hand.borrow_mut().push(10);
    let rows = add_menu(&view, cx).expect("the plus showed no menu");
    let titles: Vec<_> = rows.iter().map(|row| row.title.to_string()).collect();
    // One tab left, so no All Tabs either.
    assert_eq!(titles, [YOUR_TABS, "Eleven"]);
}

/// A chip alone is a reply: ↩ sends it with no words, the picks go with it, and a second ↩
/// sends nothing. With words the reply carries both.
#[gpui::test]
fn a_chip_and_no_words_is_a_reply(cx: &mut TestAppContext) {
    let (_, host, view, cx) = replying(cx);
    click_tab(&view, "Ten", cx);
    host.take();
    cx.simulate_keystrokes("enter");
    assert_eq!(replies(&host), [r#"task task-1 Reply("" with Picked(1))"#]);
    assert!(chips(&view, cx).is_empty(), "a pick outlived its send");
    cx.simulate_keystrokes("enter");
    assert!(replies(&host).is_empty(), "a reply of nothing");
    click_tab(&view, "Ten", cx);
    click_tab(&view, "Eleven", cx);
    cx.simulate_input("these two");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        replies(&host),
        [r#"task task-1 Reply("these two" with Picked(2))"#]
    );
}

/// A reply without a pick carries no tab, whatever its words name.
#[gpui::test]
fn typed_text_picks_no_tab_for_a_reply(cx: &mut TestAppContext) {
    let (_, host, _, cx) = replying(cx);
    cx.simulate_input("Ten 10 tab:10");
    cx.simulate_keystrokes("enter");
    assert_eq!(replies(&host), [r#"task task-1 Reply("Ten 10 tab:10")"#]);
}

/// A picked tab closed before the send loses its chip, silently, and is not in what the reply
/// carries; nothing more is asked of the core for it.
#[gpui::test]
fn a_picked_tab_closed_before_the_send_loses_its_chip(cx: &mut TestAppContext) {
    let (shell, host, view, cx) = replying(cx);
    click_tab(&view, "Ten", cx);
    click_tab(&view, "Eleven", cx);
    let asked = host.hand_asks.borrow().len();
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![11], Vec::new(), Folders::default(), cx)
    });
    cx.run_until_parked();
    assert_eq!(chips(&view, cx), [11]);
    // One gone to a page of the browser's own goes the same way.
    shell.update(cx, |shell, cx| {
        let own = Page {
            url: "ilo://settings/".into(),
            ..Page::default()
        };
        shell.set_page(11, own, cx)
    });
    cx.run_until_parked();
    assert!(chips(&view, cx).is_empty());
    assert_eq!(host.hand_asks.borrow().len(), asked);
}

/// ⌫ in the empty reply field takes the last chip; after words it is the field's.
#[gpui::test]
fn backspace_in_the_empty_reply_field_takes_the_last_chip(cx: &mut TestAppContext) {
    let (_, _, view, cx) = replying(cx);
    click_tab(&view, "Ten", cx);
    click_tab(&view, "Eleven", cx);
    cx.simulate_input("a");
    cx.simulate_keystrokes("backspace");
    assert_eq!(chips(&view, cx), [10, 11], "⌫ after words took a chip");
    cx.simulate_keystrokes("backspace");
    assert_eq!(chips(&view, cx), [10]);
    // While an input method composes, ⌫ is its own.
    let reply = view.read_with(cx, |view, _| view.reply.clone());
    cx.update(|window, cx| {
        reply.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "に", None, window, cx)
        })
    });
    cx.simulate_keystrokes("backspace");
    assert_eq!(chips(&view, cx), [10], "⌫ in a composition took a chip");
}

/// ↩ that ends an input method's composition is the input method's with a chip held too:
/// nothing is sent and the chip stays. The next ↩ sends the words and the tab together.
#[gpui::test]
fn return_ending_a_composition_sends_no_chip(cx: &mut TestAppContext) {
    let (_, host, view, cx) = replying(cx);
    click_tab(&view, "Ten", cx);
    let reply = view.read_with(cx, |view, _| view.reply.clone());
    host.take();
    cx.update(|window, cx| {
        reply.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "にほんご", None, window, cx)
        })
    });
    assert!(reply.read_with(cx, |input, _| input.composing()));
    cx.simulate_keystrokes("enter");
    assert!(replies(&host).is_empty(), "↩ while composing sent");
    assert_eq!(chips(&view, cx), [10], "↩ while composing took the chip");
    cx.update(|window, cx| {
        reply.update(cx, |input, cx| {
            input.replace_text_in_range(None, "日本語", window, cx)
        })
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(
        replies(&host),
        [r#"task task-1 Reply("日本語" with Picked(1))"#]
    );
    assert!(chips(&view, cx).is_empty());
}

/// A sent message shows its tabs as chips in its plate: the conversation draws with them, for
/// the message that started the thread with no words and for a reply alike.
#[gpui::test]
fn a_sent_messages_chips_draw_in_the_log(cx: &mut TestAppContext) {
    use crate::agents::task::handed::Chip;
    let (shell, _, view, cx) = replying(cx);
    let chip = |tab, title: &str, handed| Chip {
        tab,
        title: title.to_string().into(),
        handed,
    };
    let mut talk = conversation(1);
    talk.asked = "".into();
    talk.lines.push(Line::You("".into()));
    talk.chips = vec![
        (0, vec![chip(10, "Ten", true)]),
        (1, vec![chip(11, "Eleven", true), chip(12, "Twelve", false)]),
    ];
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    cx.run_until_parked();
    let shown = view.read_with(cx, |view, _| view.shown.as_ref().unwrap().chips.len());
    assert_eq!(shown, 2);
}
