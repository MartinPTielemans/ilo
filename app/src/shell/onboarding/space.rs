//! Your first Space (first-run-v9 §6): its name's field and the theme panel's own parts, set out as
//! the fifth mock has them, editing the first Space. The window shows each change at once and keeps
//! it as the pointer lets go, as the panel does, so a quit resumes with it; Next keeps the name, and
//! the look the step came with if nothing was changed.

use std::rc::Rc;

use gpui::{App, Context, Entity, Window, div, prelude::*, px};

use super::Step;
use super::card::Inks;
use crate::palette::Tone;
use crate::shell::Shell;
use crate::text_input::{Colors, TextInput};
use crate::theme::Theme;
use crate::theme_picker::{self, Also, Changed, Form, ThemePicker};
use crate::tokens::shell::onboarding::card as t;

/// The step's parts as the fifth mock sets them out: no padding, a head as tall as the name's
/// field, a shorter pad, and no note.
const FORM: Form = Form {
    padding: 0.,
    head: t::SPACE_EDITOR[1],
    pad_gap: t::SPACE_PAD[0],
    pad_height: t::SPACE_PAD[1],
    bars_gap: t::SPACE_PAD[2],
    bar_gap: t::SPACE_PAD[3],
    dots: t::SPACE_DOTS,
    note: false,
};

/// What the step's name is as it comes, unless an import brought Spaces (v9 §6).
const HOME: &str = "Home";

/// The step's editor, made as the step first comes and kept while the card shows, so Back and a
/// return keep what was set.
pub(in crate::shell) struct SpaceEditor {
    pub(super) space: u64,
    pub(super) name: Entity<TextInput>,
    pub(super) picker: Entity<ThemePicker>,
    /// The sheet it was made on, which it follows as the card turns light or dark.
    light: bool,
    /// The Space had no look, so the one it came with is the mock's, kept by Next.
    started: bool,
}

/// The panel's requests, which in the card it has none of: it has no window, and no trackpad's
/// detent without one (Proposed).
struct InCard;

impl theme_picker::Host for InCard {
    fn show(&self, _: u64, _: f32, _: f32, _: &[(f32, f32, f32)]) {}
    fn hide(&self) {}
    fn detent(&self) {}
}

/// The look a Space with none comes to the step with: the mock's palette, Intensity and Grain.
fn started_look() -> Option<Theme> {
    let [palette, intensity, _, grain] = t::SPACE_START;
    let made = theme_picker::palette_theme(palette as usize, None)?;
    Some(Theme {
        intensity,
        grain,
        ..made
    })
}

impl Shell {
    /// Makes the step's editor the first time the step comes, for the first Space, and turns it
    /// to the card's `light` or dark after.
    pub(super) fn space_editor(&mut self, light: bool, window: &Window, cx: &mut Context<Self>) {
        let tone = if light { Tone::Light } else { Tone::Dark };
        if let Some(editor) = self.onboarding.as_mut().and_then(|o| o.space.as_mut()) {
            if editor.light != light {
                editor.light = light;
                (editor.picker).update(cx, |picker, cx| picker.retone(tone, cx));
            }
            return;
        }
        let Some(entry) = self.spaces.first() else {
            return;
        };
        let space = entry.id;
        // After an import with Spaces the field holds the first one's name.
        let name_text: gpui::SharedString = match self.spaces.len() > 1 {
            true => entry.name.clone().into(),
            false => HOME.into(),
        };
        let dark = self.tone_of(space) == Tone::Dark;
        let had = (entry.theme.map(|theme| theme.shown(dark)))
            .or_else(|| entry.color.map(Theme::of_color));
        let started = had.is_none();
        let theme = had.or_else(started_look);
        let glass = match started {
            true => t::SPACE_START[2],
            false => self.glass_of(space),
        };
        let Some(rest) = self.onboarding.as_ref().map(|o| o.focus.clone()) else {
            return;
        };
        let shell = cx.weak_entity();
        let changed: Changed = Rc::new(move |theme, done, cx| {
            (shell.update(cx, |shell, cx| shell.theme_changed(space, theme, done, cx))).ok();
        });
        let (glassed, cleared) = (cx.weak_entity(), cx.weak_entity());
        let also = Also {
            transparency: glass,
            glassed: Rc::new(move |value, done, cx| {
                (glassed.update(cx, |shell, cx| shell.glass_changed(space, value, done, cx))).ok();
            }),
            cleared: Rc::new(move |cx| {
                (cleared.update(cx, |shell, cx| shell.remove_space_theme(space, cx))).ok();
            }),
        };
        let picker = theme_picker::inline(
            Rc::new(InCard),
            FORM,
            tone,
            space,
            theme,
            also,
            changed,
            rest,
            cx,
        );
        let ink = Inks::of(light);
        let name = cx.new(|cx| {
            TextInput::new(HOME, cx)
                .follows(move |_: &App| Colors::placeholder(gpui::Hsla::from(ink.text(t::TEXT_3))))
                .spoken("Space name")
        });
        // The name shows with the caret at its end, nothing selected, as the keyboard rests on Next.
        let caret = name_text.len();
        name.update(cx, |input, cx| input.set_text(name_text, caret, window, cx));
        // The mock's look shows on the window as the step comes; Next keeps it.
        if let Some(theme) = theme.filter(|_| started) {
            self.theme_changed(space, theme, false, cx);
            self.glass_changed(space, glass, false, cx);
        }
        // The window draws again as the editor's parts change.
        cx.observe(&picker, |_, _, cx| cx.notify()).detach();
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.space = Some(SpaceEditor {
                space,
                name,
                picker,
                light,
                started,
            });
        }
    }

    /// The step's controls: the name's field with Auto | Light | Dark beside it, the palettes, the
    /// pad and the bars, all the panel's but the field.
    pub(super) fn space_controls(&self, inks: Inks) -> gpui::Div {
        let Some(editor) = self.onboarding.as_ref().and_then(|o| o.space.as_ref()) else {
            return div();
        };
        let [_, head] = t::SPACE_EDITOR;
        let [radius, size, inset, gap] = t::SPACE_FIELD;
        let segments = 3. * crate::tokens::theme_panel::SEGMENT_WIDTH
            + 2. * crate::tokens::theme_panel::SEGMENT_INSET;
        let field = div()
            .id("onboarding-space-name")
            .absolute()
            .left_0()
            .top_0()
            .w(px(theme_picker::WIDTH - segments - gap))
            .h(px(head))
            .px(px(inset))
            .flex()
            .items_center()
            .rounded(px(radius))
            .bg(inks.field())
            .text_size(px(size))
            .text_color(inks.text(1.))
            .child(editor.name.clone())
            .relative()
            .children(self.place(Step::Space, "name"));
        div()
            .relative()
            .w(px(theme_picker::WIDTH))
            .h(px(FORM.height()))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .child(editor.picker.clone())
                    .children(self.place(Step::Space, "editor")),
            )
            .child(field)
    }

    /// Next: the name typed, cut to the field's most, the Space's if it's changed, an empty one
    /// being Home; and the look the step came with, kept, if nothing was changed.
    pub(super) fn space_keep(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.onboarding.as_ref().and_then(|o| o.space.as_ref()) else {
            return;
        };
        let (space, started) = (editor.space, editor.started);
        let typed = editor.name.read(cx).committed();
        let typed: String = typed.trim().chars().take(t::SPACE_NAME as usize).collect();
        let name = if typed.is_empty() { HOME.into() } else { typed };
        let shown = editor.picker.read(cx).editing().map(|(_, theme)| theme);
        let glass = editor.picker.read(cx).glass();
        let kept = self.spaces.iter().find(|each| each.id == space);
        if kept.is_some_and(|kept| kept.name != name) {
            log!("browser: onboarding space named");
            self.browser.rename_space(space, &name, cx);
        }
        if started {
            if let Some(Some(theme)) = shown {
                self.theme_changed(space, theme, true, cx);
            }
            if let Some(glass) = glass {
                self.glass_changed(space, glass, true, cx);
            }
        }
    }

    /// ← and →, with the keyboard on the step: the palette before or after the one chosen.
    pub(super) fn space_palette_by(&mut self, by: isize, cx: &mut Context<Self>) {
        if let Some(editor) = self.onboarding.as_ref().and_then(|o| o.space.as_ref()) {
            (editor.picker.clone()).update(cx, |picker, cx| picker.palette_by(by, cx));
        }
    }

    /// Picks palette `index` at the Space step, as a click on its disc does, for the preview's
    /// captures.
    pub fn onboarding_pick_palette(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = self.onboarding.as_ref().and_then(|o| o.space.as_ref()) {
            (editor.picker.clone()).update(cx, |picker, cx| {
                picker.palette(index, cx);
            });
        }
    }
}
