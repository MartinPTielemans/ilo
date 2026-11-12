//! What a 2-set (dubeolsik) Korean keyboard's input method asks of a field, key by key: AppKit's,
//! IBus's and Fcitx's through text-input-v3 or XIM, and Windows', which GPUI turns into the same
//! two calls. Each key rebuilds the syllable being composed. A key that can't join it commits the
//! syllable and starts the next, and a vowel after a final consonant takes that consonant with it
//! into the next syllable (핫 then ㅔ gives 하 and 세), so one key commits and marks at once.

use gpui::{Context, EntityInputHandler, Window};

use super::TextInput;

/// One call the input method makes.
#[derive(Clone, Copy, Debug)]
pub enum Ime {
    /// It composes this in place of what it composed before.
    Mark(&'static str),
    /// It commits this in place of what it composed.
    Commit(&'static str),
}

use Ime::{Commit, Mark};

/// "한글" from ㅎ ㅏ ㄴ ㄱ ㅡ ㄹ, its last syllable still composing.
pub const HANGEUL: &[Ime] = &[
    Mark("ㅎ"),
    Mark("하"),
    Mark("한"),
    Commit("한"),
    Mark("ㄱ"),
    Mark("그"),
    Mark("글"),
];

/// "안녕하세요" from ㅇ ㅏ ㄴ ㄴ ㅕ ㅇ ㅎ ㅏ ㅅ ㅔ ㅇ ㅛ, its last syllable still composing. ㅅ and the
/// second ㅇ each end one syllable, then move to the next as its vowel comes.
pub const ANNYEONG: &[Ime] = &[
    Mark("ㅇ"),
    Mark("아"),
    Mark("안"),
    Commit("안"),
    Mark("ㄴ"),
    Mark("녀"),
    Mark("녕"),
    Commit("녕"),
    Mark("ㅎ"),
    Mark("하"),
    Mark("핫"),
    Commit("하"),
    Mark("세"),
    Mark("셍"),
    Commit("세"),
    Mark("요"),
];

impl Ime {
    pub fn apply(self, input: &mut TextInput, window: &mut Window, cx: &mut Context<TextInput>) {
        match self {
            Mark(text) => input.replace_and_mark_text_in_range(None, text, None, window, cx),
            Commit(text) => input.replace_text_in_range(None, text, window, cx),
        }
    }
}
