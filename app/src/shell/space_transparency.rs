//! Transparency one per Space in the window (crate::space_transparency): today's level carried into
//! every Space once, and Settings' row and the onboarding's step setting every Space at once, until
//! the theme editor's bar takes their place.

use gpui::Context;

use super::undo::{Prop, Value};
use super::{Shell, Space};
use crate::space_transparency::{carry_in, of_level, with};
use crate::transparency;

impl Space {
    /// The theme text the core keeps for it: its theme, and its value beside it.
    pub(super) fn theme_text(&self, transparency: Option<f32>) -> String {
        let theme = self.theme.map_or_else(String::new, |theme| theme.to_text());
        with(&theme, transparency)
    }
}

impl Shell {
    /// Today's level into every Space while none keeps a value, at the first Spaces a window is
    /// given after the launch has read the level.
    pub(super) fn carry_transparency(&mut self, cx: &mut Context<Self>) {
        let Some(level) = transparency::to_carry(cx) else {
            return;
        };
        let texts: Vec<_> = (self.spaces.iter())
            .map(|space| (space.id, space.theme_text(space.transparency)))
            .collect();
        for (space, text) in carry_in(level, &texts) {
            self.browser.set_space_theme(space, &text, cx);
        }
    }

    /// The theme panel moved `space`'s glass to `value`: the window shows it at once, and once the
    /// user is `done` the core keeps it beside the Space's theme, as one step for ⌘Z: a drag is
    /// one, however many frames it took.
    pub(super) fn glass_changed(
        &mut self,
        space: u64,
        value: f32,
        done: bool,
        cx: &mut Context<Self>,
    ) {
        self.glass_preview = Some((space, value));
        if done {
            let value = Value::Text(format!("{value:.3}"));
            self.change(Prop::SpaceTransparency(space), value, cx);
        }
        cx.notify();
    }

    /// The Spaces came from the core: the glass being made gives way to the Space's own once the
    /// core has it, or the Space has gone.
    pub(super) fn settle_glass_preview(&mut self) {
        let Some((space, glass)) = self.glass_preview else {
            return;
        };
        let kept = self.spaces.iter().find(|each| each.id == space);
        if kept.is_none_or(|kept| {
            kept.transparency
                .is_some_and(|kept| (kept - glass).abs() < 5e-4)
        }) {
            self.glass_preview = None;
        }
    }

    /// The level Settings' row or the onboarding's step picked, given to every Space.
    pub(super) fn transparency_picked(&mut self, cx: &mut Context<Self>) {
        let value = Some(of_level(transparency::picked(cx)));
        let browser = self.browser.clone();
        for space in self
            .spaces
            .iter()
            .filter(|space| space.transparency != value)
        {
            browser.set_space_theme(space.id, &space.theme_text(value), cx);
        }
    }
}

#[cfg(test)]
mod tests;
