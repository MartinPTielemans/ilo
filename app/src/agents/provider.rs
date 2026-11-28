//! What each provider says of itself, which the UI reads and never matches on (#4394): who makes
//! its models, what its assistant is called, its maker's mark and the efforts its program takes.
//! Today a table keyed by provider id; slice 5 (#4465) makes each row its adapter's Caps, and a new
//! provider is one more row, with no change to a menu, chip or Settings row.

use crate::shell::MakerMark;
use crate::tokens::menu::{
    CLAUDE_MARK, CLAUDE_MARK_INK, OPENAI_MARK, OPENAI_MARK_DARK, OPENAI_MARK_LIGHT,
};

/// One provider's own data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caps {
    /// The id ilo keeps it by, as Settings and kept threads name it.
    pub id: &'static str,
    /// Who makes its models, which a screen reader says before each of them (Z54).
    pub maker: &'static str,
    /// Its maker's name for its assistant, as its rows say it, in text only.
    pub assistant: &'static str,
    /// Its maker's mark, before each of its models' names, as its maker publishes it (assets.rs
    /// MAKERS).
    pub mark: MakerMark,
    /// The effort levels its program takes at start, under the names its help gives them, in
    /// order; none where it takes no effort setting (#4140).
    pub efforts: &'static [&'static str],
}

/// The providers ilo has, in Settings' order.
pub const PROVIDERS: &[Caps] = &[
    Caps {
        id: "claude",
        maker: "Anthropic",
        assistant: "Claude Code",
        mark: MakerMark {
            path: "makers/claude-spark.svg",
            size: CLAUDE_MARK,
            light: CLAUDE_MARK_INK,
            dark: CLAUDE_MARK_INK,
        },
        // Claude Code's `--effort` lists these.
        efforts: &["low", "medium", "high", "xhigh", "max"],
    },
    Caps {
        id: "codex",
        maker: "OpenAI",
        assistant: "Codex",
        mark: MakerMark {
            path: "makers/openai-blossom.svg",
            size: OPENAI_MARK,
            light: OPENAI_MARK_LIGHT,
            dark: OPENAI_MARK_DARK,
        },
        // Codex's help names no effort option.
        efforts: &[],
    },
];

/// The provider kept by `id`, none for an id ilo has no provider by.
pub fn caps(id: &str) -> Option<&'static Caps> {
    PROVIDERS.iter().find(|caps| caps.id == id)
}
