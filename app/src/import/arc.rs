//! Arc's sidebar, read from its `StorableSidebar.json`: the Spaces with their names, icons,
//! themes and profiles, each Space's pinned tabs and folders, and each profile's favorites; and
//! its Air Traffic Control rules, from `StorableLinkRouting.json` beside it.
//!
//! The file is Swift's Codable encoding, as the public readers of it describe it (radian,
//! notarc): a dictionary is a flat array of key, value, key, value; an enum is an object with one
//! key, its value under `_0`; a colour is `red`, `green` and `blue` in extended sRGB, which can
//! leave 0 to 1. Every node of the sidebar is an item, found by its id, and a node's children are
//! its `childrenIds`, in order, some of which may name items that are gone.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::found::{Found, FromProfile, LinkRule, Node, Space, Tab, opens_here};
use crate::link_rules::Condition;
use crate::theme::{Appearance, MAX_COLORS, Place, Texture, Theme, place_of};

/// The sidebar `json` holds, with the link rules of `routing`, Arc's routing file, if it has one,
/// or why the sidebar can't be read.
pub fn parse(json: &str, routing: Option<&str>) -> Result<Found, String> {
    let root: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let containers = root
        .pointer("/sidebar/containers")
        .and_then(Value::as_array);
    // The first is `{"global": {}}`; Little Arc can add others, so the one with Spaces is it.
    let container = containers
        .and_then(|all| all.iter().find(|each| each.get("spaces").is_some()))
        .ok_or("no Spaces in Arc's sidebar")?;
    let items: HashMap<&str, &Value> = objects(container.get("items"))
        .filter_map(|item| Some((item.get("id")?.as_str()?, item)))
        .collect();
    let reader = Reader { items };
    let spaces: Vec<&Value> = objects(container.get("spaces")).collect();
    let ids: Vec<&str> = spaces
        .iter()
        .map(|space| space.get("id").and_then(Value::as_str).unwrap_or_default())
        .collect();
    let spaces = spaces.into_iter().map(|space| reader.space(space)).collect();
    let tops = container
        .get("topAppsContainerIDs")
        .and_then(Value::as_array);
    let favorites = tops
        .into_iter()
        .flat_map(|tops| tops.chunks(2))
        .filter_map(|pair| {
            let profile = profile(pair.first()?)?;
            let tabs = reader.tabs(pair.get(1)?.as_str()?);
            Some((profile, tabs))
        })
        .collect();
    Ok(Found {
        spaces,
        favorites,
        link_rules: routing.map(|routing| link_rules(routing, &ids)).unwrap_or_default(),
        ..Found::default()
    })
}

/// The rules of Arc's routing file `json` that can be ours, in its order, each to the Space of
/// `spaces`, Arc's Spaces' ids, it names. Ours match a link's URL alone and open it in a Space, so
/// a rule that matches the app a link came from, or sends it to Little Arc or the Space in front,
/// stays out, as does a file that can't be read.
///
/// A rule is `{"sourceComponents": [{"componentType": {"urlMatch": {"_0": {"contains": {}}, "_1":
/// "text"}}}], "destination": {"space": {"_0": …}}}`, as the public readers of it describe it
/// (vane, notarc), who read a Space's id either as `{"specific": {"_0": id}}` or as the key itself
/// and "is equal to" as `isEqualTo` or `exact`, so either comes.
fn link_rules(json: &str, spaces: &[&str]) -> Vec<LinkRule> {
    let Ok(root) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let rule = |rule: &Value| -> Option<LinkRule> {
        let [component] = rule.get("sourceComponents")?.as_array()?.as_slice() else {
            return None;
        };
        let url = component.pointer("/componentType/urlMatch")?;
        let condition = match only_key(url.get("_0")?)? {
            "contains" => Condition::Contains,
            "isEqualTo" | "exact" => Condition::Is,
            _ => return None,
        };
        let text = url.get("_1")?.as_str()?.trim();
        let to = rule.pointer("/destination/space/_0")?;
        let id = match only_key(to)? {
            "specific" => {
                let id = to.pointer("/specific/_0")?;
                id.as_str().or_else(|| id.get("_0")?.as_str())?
            }
            key => key,
        };
        Some(LinkRule {
            condition,
            text: text.into(),
            space: spaces.iter().position(|each| !each.is_empty() && *each == id)?,
        })
    };
    let rules = root.get("rules").and_then(Value::as_array).into_iter().flatten();
    rules.filter_map(rule).filter(|rule| !rule.text.is_empty()).collect()
}

/// The one key of an enum's object, as Swift encodes a case.
fn only_key(value: &Value) -> Option<&str> {
    let object = value.as_object()?;
    (object.len() == 1).then(|| object.keys().next().map(String::as_str))?
}

/// The objects of a flat array of ids and objects, skipping the ids.
fn objects(array: Option<&Value>) -> impl Iterator<Item = &Value> {
    array
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|each| each.is_object())
}

/// `{"default": true}` or `{"custom": {"_0": {"directoryBasename": …}}}`.
fn profile(value: &Value) -> Option<FromProfile> {
    if value.get("default").is_some() {
        return Some(FromProfile::Default);
    }
    let dir = value.pointer("/custom/_0/directoryBasename")?.as_str()?;
    Some(FromProfile::of(dir))
}

struct Reader<'a> {
    items: HashMap<&'a str, &'a Value>,
}

impl Reader<'_> {
    fn space(&self, space: &Value) -> Space {
        let name = space
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let info = space.get("customInfo");
        Space {
            name: if name.trim().is_empty() {
                "Space".into()
            } else {
                name.into()
            },
            profile: space
                .get("profile")
                .and_then(profile)
                .unwrap_or(FromProfile::Default),
            icon: info
                .and_then(|info| info.get("iconType"))
                .map(icon)
                .unwrap_or_default(),
            theme: info
                .and_then(|info| info.get("windowTheme"))
                .and_then(theme),
            pinned: pinned_container(space)
                .map(|id| self.nodes(id, &mut HashSet::new()))
                .unwrap_or_default(),
            // Today tabs stay in Arc.
            today: Vec::new(),
        }
    }

    /// The children of `id`, tabs and folders, with each split view's tabs in its place.
    fn nodes(&self, id: &str, seen: &mut HashSet<String>) -> Vec<Node> {
        // A file that names a folder inside itself would otherwise never end.
        if !seen.insert(id.into()) {
            return Vec::new();
        }
        let children = self.items.get(id).and_then(|item| item.get("childrenIds"));
        let children = children.and_then(Value::as_array).into_iter().flatten();
        let mut nodes = Vec::new();
        for child in children.filter_map(Value::as_str) {
            let Some(item) = self.items.get(child) else {
                continue;
            };
            let Some(data) = item.get("data") else {
                continue;
            };
            if let Some(tab) = data.get("tab") {
                nodes.extend(self.tab(item, tab).map(Node::Tab));
            } else if seen.contains(child) {
                continue;
            } else if data.get("list").is_some() {
                let name = item
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let name = if name.trim().is_empty() {
                    "Folder"
                } else {
                    name
                };
                nodes.push(Node::Folder {
                    name: name.into(),
                    children: self.nodes(child, seen),
                });
            } else if data.get("splitView").is_some() {
                nodes.extend(self.nodes(child, seen));
            }
        }
        nodes
    }

    /// The tabs under `id`, folders and split views flattened, as favorites have no folders.
    fn tabs(&self, id: &str) -> Vec<Tab> {
        fn flatten(nodes: Vec<Node>, tabs: &mut Vec<Tab>) {
            for node in nodes {
                match node {
                    Node::Tab(tab) => tabs.push(tab),
                    Node::Folder { children, .. } => flatten(children, tabs),
                }
            }
        }
        let mut tabs = Vec::new();
        flatten(self.nodes(id, &mut HashSet::new()), &mut tabs);
        tabs
    }

    /// A tab whose page can open here: Arc's own pages and other browsers' can't.
    fn tab(&self, item: &Value, tab: &Value) -> Option<Tab> {
        let url = ["savedURL", "currentURL"]
            .iter()
            .filter_map(|key| tab.get(*key)?.as_str())
            .find(|url| !url.is_empty())?;
        if !opens_here(url) {
            return None;
        }
        let name = item
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default();
        Some(Tab {
            url: url.into(),
            name: name.trim().into(),
        })
    }
}

/// The id of the item holding the Space's pinned tabs: after "pinned" in `containerIDs`, else
/// after the object keyed "pinned" in `newContainerIDs`.
fn pinned_container(space: &Value) -> Option<&str> {
    let after = |key: &str, is: &dyn Fn(&Value) -> bool| {
        let ids = space.get(key)?.as_array()?;
        let at = ids.iter().position(is)?;
        ids.get(at + 1)?.as_str()
    };
    after("containerIDs", &|each| each.as_str() == Some("pinned"))
        .or_else(|| after("newContainerIDs", &|each| each.get("pinned").is_some()))
}

/// An emoji as it is; one of Arc's icons as the glyph of ours that draws the same thing, else none.
fn icon(icon: &Value) -> String {
    if let Some(emoji) = icon.get("emoji_v2").and_then(Value::as_str) {
        return emoji.into();
    }
    if let Some(code) = icon.get("emoji").and_then(Value::as_u64) {
        return char::from_u32(code as u32)
            .map(String::from)
            .unwrap_or_default();
    }
    let Some(name) = icon.get("icon").and_then(Value::as_str) else {
        return String::new();
    };
    // Arc's names that differ from ours; the rest are tried as they are.
    let ours = match name {
        "flash" | "bolt" => "bolt",
        "musicalNote" | "music" => "music",
        "restaurant" | "forkKnife" => "fork-knife",
        "airplane" | "plane" => "airplane",
        "envelope" | "mail" => "envelope",
        "book" | "books" => "book",
        "pawprint" | "paw" => "paw",
        "flame" | "campfire" => "campfire",
        "person2" | "people" => "people",
        "message" | "bubble" => "chat",
        "doc" | "page" => "page",
        "terminal" => "terminal",
        "wrench" | "hammer" => "tools",
        other => other,
    };
    let known = crate::space_icon::GLYPHS
        .iter()
        .position(|(each, _)| *each == ours);
    known.map(crate::space_icon::glyph).unwrap_or_default()
}

/// The theme of Arc's `windowTheme`: one look, or a light and a dark one, and its appearance.
/// `windowAppearance` forces light or dark; without it, two looks follow the system, and one
/// looks as its `translucencyStyle` says, else follows the system too, as Ardia reads Arc's.
fn theme(window_theme: &Value) -> Option<Theme> {
    let background = window_theme.get("background")?;
    let single = background.pointer("/single/_0").and_then(look);
    let based = background.get("appearanceBased");
    let light = single.or_else(|| based?.get("light").and_then(look));
    let dark = match single {
        Some(_) => None,
        None => based.and_then(|based| based.get("dark")).and_then(look),
    };
    let (main, style) = light.or(dark)?;
    let two = light.is_some() && dark.is_some();
    let appearance = window_appearance(window_theme.get("windowAppearance"))
        .unwrap_or(if two { Appearance::Automatic } else { style });
    Some(match (appearance, light, dark) {
        (Appearance::Automatic, Some(_), Some((dark, _))) => Theme { dark: Some(dark.look()), ..main },
        (Appearance::Dark, _, Some((dark, _))) => Theme { appearance, ..dark },
        _ => Theme { appearance, ..main },
    })
}

/// `{"single": {"_0": "light"}}` forces light, and `{"appearanceBased": {"light": "light",
/// "dark": "dark"}}` follows the system; one that says the same for both forces that.
fn window_appearance(value: Option<&Value>) -> Option<Appearance> {
    let named = |value: Option<&Value>| match value?.as_str()? {
        "light" => Some(Appearance::Light),
        "dark" => Some(Appearance::Dark),
        _ => None,
    };
    let value = value?;
    if let Some(single) = named(value.pointer("/single/_0")) {
        return Some(single);
    }
    let based = value.get("appearanceBased")?;
    let (light, dark) = (named(based.get("light"))?, named(based.get("dark"))?);
    Some(if light == dark { light } else { Appearance::Automatic })
}

/// One of Arc's looks: its first colour as our main one, as many colours as it has up to ours,
/// counting a gradient's third, overlaid one, its intensity, its grain and our texture for its;
/// and the appearance its `translucencyStyle` names, else automatic. Arc places its colours
/// freely, as ours can be, so each of the others is placed in the field at its own hue and
/// lightness, at the main one's saturation, which is the theme's, and the third also comes
/// exactly, as it lies over the other two. A basic colour is one colour with no grain.
fn look(background: &Value) -> Option<(Theme, Appearance)> {
    let style = background.pointer("/style/color/_0")?;
    let (colors, config) = if let Some(gradient) = style.pointer("/blendedGradient/_0") {
        let mut colors: Vec<u32> = gradient
            .get("baseColors")?
            .as_array()?
            .iter()
            .filter_map(color)
            .collect();
        // Arc's third colour is the second of its overlay colours, the first being clear.
        let overlay = gradient.get("overlayColors").and_then(Value::as_array);
        if let Some(third) = overlay.and_then(|overlay| overlay.get(1)).and_then(color) {
            colors.push(third);
        }
        (colors, gradient)
    } else {
        let single = style.pointer("/blendedSingleColor/_0").or_else(|| style.pointer("/basicColor/_0"))?;
        (vec![color(single.get("color")?)?], single)
    };
    let main = *colors.first()?;
    let basic = style.get("basicColor").is_some();
    let factor = |key: &str| {
        config
            .get("modifiers")?
            .get(key)?
            .as_f64()
            .map(|value| value.clamp(0., 1.) as f32)
    };
    let appearance = match config.get("translucencyStyle").and_then(Value::as_str) {
        Some("dark") => Appearance::Dark,
        Some("light") => Appearance::Light,
        _ => Appearance::Automatic,
    };
    let theme = Theme::of_color(main).with_count(colors.len().min(MAX_COLORS));
    let overlay = config.get("modifiers").and_then(|modifiers| modifiers.get("overlay"));
    let theme = Theme {
        intensity: factor("intensityFactor").unwrap_or(theme.intensity),
        grain: if basic { 0. } else { factor("noiseFactor").unwrap_or(0.) },
        texture: overlay.and_then(Value::as_str).map_or(Texture::Noise, texture),
        third: colors.get(2).copied().filter(|_| theme.count == MAX_COLORS),
        places: places(&theme, &colors),
        ..theme
    };
    Some((theme, appearance))
}

/// Where `colors` after the main one go round `theme`'s main one, each at its own place in the
/// field; none for a theme of one colour.
fn places(theme: &Theme, colors: &[u32]) -> Option<[Place; 2]> {
    let place = |index: usize| match colors.get(index) {
        Some(&color) => {
            let at = Theme::of_color(color);
            place_of(theme.x, theme.y, at.x, at.y)
        }
        None => theme.place(index),
    };
    (theme.count > 1).then(|| [place(1), place(2)])
}

/// Ours for Arc's texture `name`; its grain, or one it doesn't have yet, as noise.
fn texture(name: &str) -> Texture {
    match name {
        "sand" => Texture::Dust,
        "denim" => Texture::Weave,
        "tweed" => Texture::Fleck,
        _ => Texture::Noise,
    }
}

/// 0xRRGGBB of an Arc colour, its extended sRGB brought into sRGB.
fn color(color: &Value) -> Option<u32> {
    let channel = |key: &str| {
        let value = color.get(key)?.as_f64()?;
        Some((value.clamp(0., 1.) * 255.).round() as u32)
    };
    Some(channel("red")? << 16 | channel("green")? << 8 | channel("blue")?)
}
