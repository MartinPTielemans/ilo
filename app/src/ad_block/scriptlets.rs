//! uBlock Origin's scriptlets, which `+js(...)` filters run in a page, read from the modules in its
//! js/resources. Each registers a function with `registerScriptlet(fn, { name, aliases,
//! dependencies, requiresTrust })`, and uBlock Origin injects each function's own source, as
//! `fn.toString()` gives it, after those it depends on. adblock-rust takes the same: a resource a
//! function, named as uBlock Origin names it, its dependencies by name. So this reads the modules
//! only as far as it needs to: each top-level function's and class's source and each registration,
//! skipping strings, templates, regular expressions and comments, whose braces aren't the code's.
//! A few it registers are its own code's, which the modules import from js/.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use adblock::resources::{MimeType, PermissionMask, Resource, ResourceType};
use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;

use super::TRUSTED;

/// The scriptlets in uBlock Origin's `folder`, as adblock-rust's resources. uBlock Origin runs a few
/// in the page's isolated world; these all run in its main world, as Brave runs them, since
/// adblock-rust has no worlds, and they touch only the page's DOM.
pub fn scriptlets(folder: &Path) -> Vec<Resource> {
    let mut sources = Vec::new();
    if let Ok(entries) = fs::read_dir(folder.join("js").join("resources")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "js") {
                if let Ok(source) = fs::read_to_string(&path) {
                    sources.push(source);
                }
            }
        }
    }
    let mut imported: Vec<String> = sources.iter().flat_map(|source| imports(source)).collect();
    imported.sort();
    imported.dedup();
    for module in imported {
        if let Ok(source) = fs::read_to_string(folder.join("js").join(module)) {
            sources.push(source);
        }
    }
    resources(&sources)
}

/// The modules of js/ that `source`, a module of js/resources, imports from: `from '../name.js'`.
fn imports(source: &str) -> Vec<String> {
    let tokens = lex(source, 0).0;
    (tokens.windows(2))
        .filter_map(|pair| match (&pair[0].0, &pair[1].0) {
            (Token::Word("from"), Token::Text(path)) => path.strip_prefix("../"),
            _ => None,
        })
        .filter(|name| !name.contains('/') && name.ends_with(".js"))
        .map(str::to_string)
        .collect()
}

/// What `registerScriptlet` was given.
#[derive(Debug, Default)]
struct Registered {
    function: String,
    name: String,
    aliases: Vec<String>,
    dependencies: Vec<String>,
    trusted: bool,
}

/// The resources registered in the modules `sources`, each with its function's source. A
/// registration whose function isn't a function or class declaration in them is left out.
fn resources(sources: &[String]) -> Vec<Resource> {
    let mut functions = HashMap::new();
    let mut registered = Vec::new();
    for source in sources {
        read(source, &mut functions, &mut registered);
    }
    // A dependency is the function itself, which uBlock Origin names as it was registered.
    let names: HashMap<&str, &str> = (registered.iter())
        .map(|each| (each.function.as_str(), each.name.as_str()))
        .collect();
    (registered.iter())
        .filter_map(|each| {
            let source = functions.get(&each.function)?;
            Some(Resource {
                name: each.name.clone(),
                aliases: each.aliases.clone(),
                kind: ResourceType::Mime(match each.name.ends_with(".fn") {
                    true => MimeType::FnJavascript,
                    false => MimeType::ApplicationJavascript,
                }),
                content: BASE64_STANDARD.encode(source),
                dependencies: (each.dependencies.iter())
                    .map(|dependency| {
                        names
                            .get(dependency.as_str())
                            .unwrap_or(&dependency.as_str())
                            .to_string()
                    })
                    .collect(),
                permission: PermissionMask::from_bits(if each.trusted { TRUSTED } else { 0 }),
            })
        })
        .collect()
}

#[derive(Debug, PartialEq)]
enum Token<'a> {
    Word(&'a str),
    Text(String),
    Mark(u8),
    /// A template, a regular expression, or a character outside ASCII.
    Other,
}

/// A token, with where it starts and ends in the source.
type Spanned<'a> = (Token<'a>, usize, usize);

/// Adds the top-level functions and classes of `source` to `functions`, by name, and its
/// registrations to `registered`.
fn read(source: &str, functions: &mut HashMap<String, String>, registered: &mut Vec<Registered>) {
    let tokens = lex(source, 0).0;
    let mut depth = 0usize;
    let mut at = 0;
    while at < tokens.len() {
        match &tokens[at].0 {
            Token::Mark(b'{') => depth += 1,
            Token::Mark(b'}') => depth = depth.saturating_sub(1),
            Token::Word(word @ ("function" | "class")) if depth == 0 => {
                let declared = match *word {
                    "function" => function(&tokens, at),
                    _ => class(&tokens, at),
                };
                if let Some((name, end)) = declared {
                    // An async function's source starts at its async.
                    let start = match at.checked_sub(1).map(|before| &tokens[before].0) {
                        Some(Token::Word("async")) => tokens[at - 1].1,
                        _ => tokens[at].1,
                    };
                    functions.insert(name.to_string(), source[start..end].to_string());
                }
            }
            Token::Word("registerScriptlet") => {
                if let Some(each) = registration(&tokens, at) {
                    registered.push(each);
                }
            }
            _ => {}
        }
        at += 1;
    }
}

/// The name of the function declared at `tokens[at]`, `function`, and where its body ends.
fn function<'a>(tokens: &[Spanned<'a>], at: usize) -> Option<(&'a str, usize)> {
    let Some((Token::Word(name), ..)) = tokens.get(at + 1) else {
        return None;
    };
    let parameters = closing(tokens, at + 2, b'(', b')')?;
    let body = closing(tokens, parameters + 1, b'{', b'}')?;
    Some((name, tokens[body].2))
}

/// The name of the class declared at `tokens[at]`, `class`, and where its body ends.
fn class<'a>(tokens: &[Spanned<'a>], at: usize) -> Option<(&'a str, usize)> {
    let Some((Token::Word(name), ..)) = tokens.get(at + 1) else {
        return None;
    };
    let body = closing(tokens, at + 2, b'{', b'}')?;
    Some((name, tokens[body].2))
}

/// The index of the token that closes the `open` at `tokens[at]`.
fn closing(tokens: &[Spanned], at: usize, open: u8, close: u8) -> Option<usize> {
    if tokens.get(at).map(|token| &token.0) != Some(&Token::Mark(open)) {
        return None;
    }
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate().skip(at) {
        match token.0 {
            Token::Mark(mark) if mark == open => depth += 1,
            Token::Mark(mark) if mark == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// The registration at `tokens[at]`, `registerScriptlet(fn, { ... })`.
fn registration(tokens: &[Spanned], at: usize) -> Option<Registered> {
    let (Some(Token::Mark(b'(')), Some(Token::Word(function)), Some(Token::Mark(b','))) = (
        tokens.get(at + 1).map(|token| &token.0),
        tokens.get(at + 2).map(|token| &token.0),
        tokens.get(at + 3).map(|token| &token.0),
    ) else {
        return None;
    };
    let end = closing(tokens, at + 4, b'{', b'}')?;
    let mut each = Registered {
        function: function.to_string(),
        ..Default::default()
    };
    let mut index = at + 5;
    while index + 1 < end {
        if let (Token::Word(key), Token::Mark(b':')) = (&tokens[index].0, &tokens[index + 1].0) {
            let value = &tokens[index + 2].0;
            match (*key, value) {
                ("name", Token::Text(text)) => each.name = text.clone(),
                ("requiresTrust", Token::Word(word)) => each.trusted = *word == "true",
                ("aliases" | "dependencies", Token::Mark(b'[')) => {
                    let close = closing(tokens, index + 2, b'[', b']')?;
                    let items =
                        (tokens[index + 3..close].iter()).filter_map(|token| match &token.0 {
                            Token::Word(word) => Some(word.to_string()),
                            Token::Text(text) => Some(text.clone()),
                            _ => None,
                        });
                    match *key {
                        "aliases" => each.aliases = items.collect(),
                        _ => each.dependencies = items.collect(),
                    }
                    index = close;
                }
                _ => {}
            }
        }
        index += 1;
    }
    (!each.name.is_empty()).then_some(each)
}

/// Words that end where an expression can start, so a `/` after them starts a regular expression.
const BEFORE_EXPRESSION: &[&str] = &[
    "return",
    "typeof",
    "instanceof",
    "in",
    "of",
    "new",
    "delete",
    "void",
    "throw",
    "case",
    "do",
    "else",
    "yield",
    "await",
];

/// The tokens of the code at `source[start..]`, and where it stopped: at its end, or after a `}`
/// it didn't open, which ends a template's `${...}`.
fn lex(source: &str, start: usize) -> (Vec<Spanned<'_>>, usize) {
    let bytes = source.as_bytes();
    let mut tokens: Vec<Spanned> = Vec::new();
    let mut depth = 0usize;
    let mut at = start;
    while at < bytes.len() {
        let byte = bytes[at];
        let next = bytes.get(at + 1).copied();
        let from = at;
        match byte {
            b' ' | b'\t' | b'\n' | b'\r' => at += 1,
            b'/' if next == Some(b'/') => {
                at = (source[at..].find('\n')).map_or(bytes.len(), |line| at + line);
            }
            b'/' if next == Some(b'*') => {
                at = (source[at + 2..].find("*/")).map_or(bytes.len(), |end| at + 2 + end + 2);
            }
            b'\'' | b'"' => {
                let (text, end) = quoted(source, at);
                tokens.push((Token::Text(text), from, end));
                at = end;
            }
            b'`' => {
                at = template(source, at);
                tokens.push((Token::Other, from, at));
            }
            b'/' if expression_starts(tokens.last().map(|token| &token.0)) => {
                at = expression(bytes, at);
                tokens.push((Token::Other, from, at));
            }
            b'{' => {
                depth += 1;
                tokens.push((Token::Mark(byte), from, at + 1));
                at += 1;
            }
            b'}' if depth == 0 && start > 0 => return (tokens, at + 1),
            b'}' => {
                depth = depth.saturating_sub(1);
                tokens.push((Token::Mark(byte), from, at + 1));
                at += 1;
            }
            _ if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' => {
                while at < bytes.len()
                    && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_' || bytes[at] == b'$')
                {
                    at += 1;
                }
                tokens.push((Token::Word(&source[from..at]), from, at));
            }
            _ if byte.is_ascii() => {
                tokens.push((Token::Mark(byte), from, at + 1));
                at += 1;
            }
            _ => {
                at += source[at..].chars().next().map_or(1, char::len_utf8);
                tokens.push((Token::Other, from, at));
            }
        }
    }
    (tokens, at)
}

/// Whether a `/` after `last` starts a regular expression rather than divides.
fn expression_starts(last: Option<&Token>) -> bool {
    match last {
        None => true,
        Some(Token::Mark(mark)) => !matches!(mark, b')' | b']' | b'}'),
        Some(Token::Word(word)) => BEFORE_EXPRESSION.contains(word),
        Some(_) => false,
    }
}

/// The string quoted at `source[at]`, unescaped as far as names need, and where it ends.
fn quoted(source: &str, at: usize) -> (String, usize) {
    let bytes = source.as_bytes();
    let quote = bytes[at];
    let mut end = at + 1;
    while end < bytes.len() && bytes[end] != quote && bytes[end] != b'\n' {
        end += if bytes[end] == b'\\' { 2 } else { 1 };
    }
    let end = end.min(bytes.len());
    (
        source[at + 1..end].replace("\\'", "'"),
        (end + 1).min(bytes.len()),
    )
}

/// Where the template at `source[at]` ends, after the code of each `${...}` in it.
fn template(source: &str, at: usize) -> usize {
    let bytes = source.as_bytes();
    let mut end = at + 1;
    while end < bytes.len() {
        match bytes[end] {
            b'\\' => end += 2,
            b'`' => return end + 1,
            b'$' if bytes.get(end + 1) == Some(&b'{') => end = lex(source, end + 2).1,
            _ => end += 1,
        }
    }
    bytes.len()
}

/// Where the regular expression at `bytes[at]` ends, after its flags.
fn expression(bytes: &[u8], at: usize) -> usize {
    let mut end = at + 1;
    let mut class = false;
    while end < bytes.len() {
        match bytes[end] {
            b'\\' => end += 1,
            b'[' => class = true,
            b']' => class = false,
            b'/' if !class => break,
            b'\n' => return at + 1,
            _ => {}
        }
        end += 1;
    }
    end += 1;
    while end < bytes.len() && bytes[end].is_ascii_alphabetic() {
        end += 1;
    }
    end.min(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoded(resource: &Resource) -> String {
        String::from_utf8(BASE64_STANDARD.decode(&resource.content).unwrap()).unwrap()
    }

    #[test]
    fn a_registered_function_comes_whole_with_its_dependencies_by_name() {
        // Braces in a regular expression, a template, a string, a comment and a default argument
        // aren't the body's.
        let helpers = r#"
import { registerScriptlet } from './base.js';
export function safeSelf() {
    const re = /[{}]\}/g; // a } in a comment
    return { re, text: `${ `{` } }`, other: '}' };
}
registerScriptlet(safeSelf, {
    name: 'safe-self.fn',
});
"#;
        let main = r#"
import { safeSelf } from './safe-self.js';
function setConstant(chain = '', extra = {}) {
    const safe = safeSelf();
    if ( chain / 2 > 1 ) { return; }
    /* } */
}
registerScriptlet(setConstant, {
    name: 'set-constant.js',
    aliases: [
        'set.js',
    ],
    dependencies: [
        safeSelf,
    ],
    requiresTrust: true,
});
"#;
        let resources = resources(&[helpers.to_string(), main.to_string()]);
        assert_eq!(resources.len(), 2);
        let set = resources
            .iter()
            .find(|each| each.name == "set-constant.js")
            .unwrap();
        assert_eq!(set.aliases, ["set.js"]);
        assert_eq!(set.dependencies, ["safe-self.fn"]);
        // Only a trusted list's filter runs it.
        assert!(
            set.permission
                .is_injectable_by(PermissionMask::from_bits(TRUSTED))
        );
        assert!(!set.permission.is_injectable_by(PermissionMask::default()));
        assert!(decoded(set).starts_with("function setConstant(chain = '', extra = {}) {"));
        assert!(decoded(set).ends_with("/* } */\n}"));
        let safe = resources
            .iter()
            .find(|each| each.name == "safe-self.fn")
            .unwrap();
        assert_eq!(safe.kind, ResourceType::Mime(MimeType::FnJavascript));
        assert!(decoded(safe).ends_with("other: '}' };\n}"));
    }

    #[test]
    fn a_class_its_own_code_exports_and_an_async_function_come_whole() {
        let shared = "import { Parser as __Parser__ } from '../parser.js';
export const Parser = __Parser__;
registerScriptlet(Parser, { name: 'parser.fn' });";
        let parser = "export class Parser {
    constructor(separator) { this.separator = separator; }
    split(text) { return text.split(this.separator).map(each => `${each}`); }
}";
        let edit = "async function editFn(text) {
    const parts = new Parser('/').split(await text);
    return parts;
}
registerScriptlet(editFn, { name: 'edit.fn', dependencies: [ Parser ] });";
        assert_eq!(imports(shared), ["parser.js"]);
        let resources = resources(&[shared.into(), edit.into(), parser.into()]);
        let class = resources
            .iter()
            .find(|each| each.name == "parser.fn")
            .unwrap();
        assert!(decoded(class).starts_with("class Parser {"));
        assert!(decoded(class).ends_with("`${each}`); }\n}"));
        let edit = resources
            .iter()
            .find(|each| each.name == "edit.fn")
            .unwrap();
        assert!(decoded(edit).starts_with("async function editFn(text) {"));
        assert_eq!(edit.dependencies, ["parser.fn"]);
    }
}
