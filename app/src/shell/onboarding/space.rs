//! Your first Space (first-run-v9 §6): its name's field and the theme panel's own parts, set out as
//! the fifth mock has them, editing the first Space. The window shows each change at once and keeps
//! it as the pointer lets go, as the panel does, so a quit resumes with it; Next keeps the name. A
//! Space with no look shows the mock's while the step does, and nothing of it is kept unless picked.

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
    /// The Space had no look, so the one it shows is the mock's, kept by nothing.
    started: bool,
    /// The user picked something, which the panel's road kept as it was made.
    picked: bool,
}

/// The panel's requests, which in the card it has none of: it has no window, and no trackpad's
/// detent without one (Proposed).
struct InCard;

impl theme_picker::Host for InCard {
    fn show(&self, _: u64, _: f32, _: f32, _: &[(f32, f32, f32)]) {}
    fn hide(&self) {}
    fn detent(&self) {}
}

/// The look a Space with none shows at the step: the mock's palette, Intensity and Grain.
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
            // Back at the step after leaving it untouched, the window shows the mock's look again.
            let (space, unpicked) = (editor.space, editor.started && !editor.picked);
            if unpicked && self.theme_preview.is_none_or(|(each, _)| each != space) {
                self.space_show_started(space, cx);
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
        // The panel calls these from inside an update of the Shell's own (the step's keys, the
        // preview's pick), so each acts once that update has returned, as the import does.
        let shell = cx.weak_entity();
        let changed: Changed = Rc::new(move |theme, done, cx| {
            let shell = shell.clone();
            cx.defer(move |cx| {
                (shell.update(cx, |shell, cx| {
                    shell.space_picked(done);
                    shell.theme_changed(space, theme, done, cx)
                }))
                .ok();
            });
        });
        let (glassed, cleared) = (cx.weak_entity(), cx.weak_entity());
        let also = Also {
            transparency: glass,
            glassed: Rc::new(move |value, done, cx| {
                let glassed = glassed.clone();
                cx.defer(move |cx| {
                    (glassed.update(cx, |shell, cx| {
                        shell.space_picked(done);
                        shell.glass_changed(space, value, done, cx)
                    }))
                    .ok();
                });
            }),
            cleared: Rc::new(move |cx| {
                let cleared = cleared.clone();
                cx.defer(move |cx| {
                    (cleared.update(cx, |shell, cx| {
                        shell.space_picked(true);
                        shell.remove_space_theme(space, cx)
                    }))
                    .ok();
                });
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
        // The mock's look shows on the window as the step comes; only a pick keeps anything.
        if started {
            self.space_show_started(space, cx);
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
                picked: false,
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
    /// being Home. The look was kept as it was picked; one only shown goes with the step.
    pub(super) fn space_keep(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.onboarding.as_ref().and_then(|o| o.space.as_ref()) else {
            return;
        };
        let space = editor.space;
        let typed = editor.name.read(cx).committed();
        let typed: String = typed.trim().chars().take(t::SPACE_NAME as usize).collect();
        let name = if typed.is_empty() { HOME.into() } else { typed };
        let kept = self.spaces.iter().find(|each| each.id == space);
        if kept.is_some_and(|kept| kept.name != name) {
            log!("browser: onboarding space named");
            self.browser.rename_space(space, &name, cx);
        }
        self.space_leave(cx);
    }

    /// The step is left, by Next or Back: a look only shown, never picked, goes from the window.
    pub(super) fn space_leave(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.onboarding.as_ref().and_then(|o| o.space.as_ref()) else {
            return;
        };
        if !editor.started || editor.picked {
            return;
        }
        let space = editor.space;
        if self.theme_preview.is_some_and(|(each, _)| each == space) {
            self.theme_preview = None;
        }
        if self.glass_preview.is_some_and(|(each, _)| each == space) {
            self.glass_preview = None;
        }
        cx.notify();
    }

    /// Shows the mock's look on `space`'s window, keeping none of it.
    fn space_show_started(&mut self, space: u64, cx: &mut Context<Self>) {
        if let Some(theme) = started_look() {
            self.theme_changed(space, theme, false, cx);
        }
        self.glass_changed(space, t::SPACE_START[2], false, cx);
    }

    /// The panel set something: once the user is done, a pick, which the panel's road has kept.
    fn space_picked(&mut self, done: bool) {
        if let Some(editor) = self.onboarding.as_mut().and_then(|o| o.space.as_mut()) {
            editor.picked |= done;
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
