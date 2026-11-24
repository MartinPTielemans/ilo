//! How a surface closes, one rule for every surface in the app (#4108): it says why it closed and
//! who closed it, and gives the keyboard back to what had it as it opened. Lifted from the
//! Library's (shell/library/keys.rs), the one surface that already gave the keyboard back to the
//! element rather than to a window.
//!
//! The rule, the Library's: closed by Escape or by the key or command that opened it, the surface
//! gives the keyboard back to its opener, or to the fallback its caller names when the opener has
//! gone; closed any other way (a press outside, a choice inside, focus or the window leaving), to
//! the fallback. Either way only while the surface still has the keyboard: what the user moved
//! the keyboard to in the meantime keeps it.
//!
//! Return and Escape while an input method composes in one of the surface's fields are the
//! composition's, and the input method ends it: the surface neither submits nor closes, and
//! leaves the marked text alone. A field's Return asks `submits` and its Escape
//! `Close::escape_from`, with the surface's fields, and nothing else reads the composition for
//! them.
//!
//! How to compose it: as the surface takes the keyboard, `FocusReturn::remember` who had it
//! (the page, if its key came from there, as GPUI's focus doesn't see the page's). Every way it
//! closes makes a `Close` (`Close::escape_from` for Escape, so a composition keeps it); then,
//! while its own elements may still have the keyboard, `FocusReturn::give_back` with them and the
//! fallback, and on `Returned::Page` hand the page the keyboard as the surface may: it comes only
//! while no other window of ours, such as an overlay, has the keyboard.

use gpui::{App, Entity, FocusHandle, WeakFocusHandle, Window};

use crate::text_input::TextInput;

/// Whether Return acts on the surface: not while an input method composes in one of its
/// `fields`, as the key is the composition's.
pub fn submits<'a>(fields: impl IntoIterator<Item = &'a Entity<TextInput>>, cx: &App) -> bool {
    !composing(fields, cx)
}

fn composing<'a>(fields: impl IntoIterator<Item = &'a Entity<TextInput>>, cx: &App) -> bool {
    (fields.into_iter()).any(|field| field.read(cx).composing())
}

/// Why a surface closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// Escape.
    Escape,
    /// The key or command that opens it, again.
    Toggled,
    /// A press outside it.
    PressOutside,
    /// The keyboard went elsewhere in its window.
    FocusLeft,
    /// Its window, or the app, isn't the active one any more.
    WindowLeft,
    /// Something chosen in it, which closes it as it acts.
    Chosen,
}

/// Who closed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    Pointer,
    Keyboard,
    /// The app itself, such as the Space it belongs to going.
    Code,
    /// An agent, through the core's API.
    Agent,
}

/// A surface's close: why, and who.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Close {
    pub why: Why,
    pub by: By,
}

impl Close {
    pub const fn new(why: Why, by: By) -> Self {
        Self { why, by }
    }

    /// Escape's close, or none while an input method composes in one of the surface's `fields`,
    /// as the key is the composition's.
    pub fn escape_from<'a>(
        fields: impl IntoIterator<Item = &'a Entity<TextInput>>,
        cx: &App,
    ) -> Option<Self> {
        (!composing(fields, cx)).then_some(Self::new(Why::Escape, By::Keyboard))
    }

    /// The close of an overlay the host hid as the user left it: a press outside it, or, with no
    /// window of the app active (`app_left`), leaving the app. The host hears that the user left,
    /// not with what, so both are the pointer's.
    pub fn left(app_left: bool) -> Self {
        let why = if app_left {
            Why::WindowLeft
        } else {
            Why::PressOutside
        };
        Self::new(why, By::Pointer)
    }

    /// Whether this close gives the keyboard back to the surface's opener, rather than to the
    /// fallback: Escape and the surface's own key or command do, as they're how a user goes back
    /// to what they were doing.
    pub fn gives_back(&self) -> bool {
        matches!(self.why, Why::Escape | Why::Toggled)
    }
}

/// Who had the keyboard as a surface took it.
#[derive(Clone, Debug)]
enum Opener {
    /// The page, which GPUI's focus doesn't see.
    Page,
    /// An element of the UI, gone once its owner lets its handle go.
    Ui(WeakFocusHandle),
}

/// The keyboard's way back from a surface, from its opening to its close.
#[derive(Clone, Debug, Default)]
pub struct FocusReturn {
    opener: Option<Opener>,
}

/// Where `give_back` sent the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Returned {
    /// The surface hadn't the keyboard any more: what took it keeps it.
    Left,
    /// Back to the element that had it.
    Opener,
    /// To the fallback, and the page had it, with no other overlay of ours holding the keyboard:
    /// the caller hands it on to the page if it may.
    Page,
    /// To the fallback.
    Fallback,
}

impl FocusReturn {
    /// Remembers who has the keyboard as the surface takes it: the page if `from_page`, else the
    /// UI element focused in `window`, if any.
    pub fn remember(from_page: bool, window: &Window, cx: &App) -> Self {
        let opener = match from_page {
            true => Some(Opener::Page),
            false => window
                .focused(cx)
                .map(|focused| Opener::Ui(focused.downgrade())),
        };
        Self { opener }
    }

    /// Gives the keyboard back as the surface closes for `close`, if one of the surface's `own`
    /// elements has it still: to the opener if the close gives back and the opener's still there,
    /// else to `fallback`.
    pub fn give_back(
        self,
        close: Close,
        own: &[&FocusHandle],
        fallback: &FocusHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> Returned {
        if !own.iter().any(|handle| handle.is_focused(window)) {
            return Returned::Left;
        }
        let opener = self.opener.filter(|_| close.gives_back());
        match opener {
            Some(Opener::Ui(before)) => match before.upgrade() {
                Some(before) => {
                    window.focus(&before, cx);
                    Returned::Opener
                }
                None => {
                    window.focus(fallback, cx);
                    Returned::Fallback
                }
            },
            Some(Opener::Page) => {
                window.focus(fallback, cx);
                // Another overlay of ours, a window of its own, has the keyboard: the page
                // doesn't take it from under it.
                let elsewhere =
                    (cx.active_window()).is_some_and(|active| active != window.window_handle());
                if elsewhere {
                    Returned::Fallback
                } else {
                    Returned::Page
                }
            }
            None => {
                window.focus(fallback, cx);
                Returned::Fallback
            }
        }
    }
}

#[cfg(test)]
mod tests;
