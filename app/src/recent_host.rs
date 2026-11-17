//! The core's side of what's recent on a pinned tab's or favorite's card (#3074): the ask, and
//! its answer, to the window that asked.

use super::*;

use browser::recent::{Answer, List, Row};

thread_local! {
    /// The window that asked for each tab's rows, whose card the answer is for.
    static ASKED: RefCell<HashMap<bc::bc_id, bc::bc_id>> = RefCell::default();
}

/// Asks the core for `tab`'s rows, for `window`'s card.
pub(crate) fn fetch(window: bc::bc_id, tab: bc::bc_id) {
    ASKED.with_borrow_mut(|asked| asked.insert(tab, window));
    unsafe { bc::bc_tab_recent_fetch(tab) };
}

// Once an ask, from a task of the core's own, never inside bc_tab_recent_fetch.
pub(crate) unsafe extern "C" fn tab_recent_answered(
    _: *mut c_void,
    tab: bc::bc_id,
    answer: c_int,
    answered: i64,
    items: *const bc::bc_live_item,
    count: usize,
) {
    let answer = match answer as u32 {
        bc::BC_LIVE_ANSWERED => {
            let items = if items.is_null() {
                &[][..]
            } else {
                unsafe { std::slice::from_raw_parts(items, count) }
            };
            let text = |text: *const c_char| -> SharedString {
                if text.is_null() {
                    return SharedString::default();
                }
                unsafe { CStr::from_ptr(text) }
                    .to_string_lossy()
                    .into_owned()
                    .into()
            };
            let rows = (items.iter())
                .map(|item| Row {
                    list: if item.list as u32 == bc::BC_LIVE_REVIEW_REQUESTS {
                        List::Review
                    } else {
                        List::Created
                    },
                    repo: text(item.repo),
                    number: u64::from(item.number),
                    title: text(item.title),
                    draft: item.state as u32 == bc::BC_LIVE_DRAFT,
                })
                .collect();
            Answer::Rows {
                rows,
                answered: answered as f64,
            }
        }
        bc::BC_LIVE_SIGNED_OUT => Answer::SignedOut,
        _ => Answer::Failed,
    };
    let Some(window) = ASKED.with_borrow_mut(|asked| asked.remove(&tab)) else {
        return;
    };
    update_app_later(move |cx| {
        update_window_in(window, cx, |shell, cx| {
            shell.recent_answered(tab, answer, cx)
        });
    });
}
