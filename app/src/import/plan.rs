//! What an import adds, worked out against what the browser has, so that running it again adds
//! nothing: a Space joins the Space here with its name and profile, a folder the one with its name
//! in the same place, a tab or favorite whose URL is already there is skipped, as is an open
//! tab whose URL the Space has among its pinned or Today tabs, and so is a link rule the user has.
//! History and addresses are added whole, and the core skips the pages and addresses it has
//! (bc_profile_add_history, bc_profile_add_addresses).

use super::found::{Address, Destination, Found, FromProfile, Node, Page, Split, Tab};
use crate::link_rules::{Condition, FRONT, LITTLE, Rule};
use crate::theme::Theme;

/// What the browser has, as far as an import needs to know.
#[derive(Clone, Debug, PartialEq)]
pub struct Existing {
    /// The profiles, by id and name, the one the browser started with first.
    pub profiles: Vec<(u64, String)>,
    /// The user's Spaces, never an agent's.
    pub spaces: Vec<ExistingSpace>,
    /// Each favorite's profile and URL.
    pub favorites: Vec<(u64, String)>,
    /// The link rules, in order.
    pub link_rules: Vec<Rule>,
    /// Where everything else opens, as `link_rules::Rules` holds it.
    pub link_otherwise: u64,
}

impl Default for Existing {
    fn default() -> Existing {
        Existing {
            profiles: Vec::new(),
            spaces: Vec::new(),
            favorites: Vec::new(),
            link_rules: Vec::new(),
            link_otherwise: LITTLE,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExistingSpace {
    pub id: u64,
    pub name: String,
    pub profile: u64,
    pub pinned: Vec<Row>,
    /// Its Today tabs' URLs.
    pub today: Vec<String>,
}

/// A pinned row: a tab by the URL it's pinned at, or a folder with what's in it.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Tab(String),
    Folder {
        id: u64,
        name: String,
        rows: Vec<Row>,
    },
}

/// A profile an import puts things in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProfileRef {
    Existing(u64),
    /// The plan's `new_profiles` at this index, made by the import.
    New(usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub new_profiles: Vec<String>,
    pub spaces: Vec<SpacePlan>,
    /// Favorites to add, each with the profile it's for.
    pub favorites: Vec<(ProfileRef, Tab)>,
    /// Each profile's history.
    pub history: Vec<(ProfileRef, Vec<Page>)>,
    /// Each profile's addresses.
    pub addresses: Vec<(ProfileRef, Vec<Address>)>,
    /// Link rules to add after the user's, in order.
    pub link_rules: Vec<PlannedRule>,
    /// Where everything else is to open, if the import changes it.
    pub link_otherwise: Option<SpaceRef>,
}

/// A link rule to add, to a Space that's here or one the plan makes.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedRule {
    pub condition: Condition,
    pub text: String,
    pub space: SpaceRef,
}

/// Where a link rule goes: a Space, or in place of one, Little ilo or the Space in front.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpaceRef {
    Existing(u64),
    /// The plan's `spaces` at this index.
    Planned(usize),
    Little,
    Front,
}

/// How much a plan brings in.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts {
    /// Spaces it makes.
    pub spaces: usize,
    pub pinned: usize,
    /// Tabs it opens among the Today tabs.
    pub today: usize,
    pub favorites: usize,
    /// Pages of history, which may be here already.
    pub pages: usize,
    /// Addresses, which may be here already.
    pub addresses: usize,
    /// Link rules, which aren't here already, and where everything else opens if it changes.
    pub link_rules: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpacePlan {
    pub into: Target,
    /// What to add to its pinned tabs, at their ends.
    pub pinned: Vec<Add>,
    /// What to open at the top of its Today tabs, in this order.
    pub today: Vec<Tab>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Existing(u64),
    New {
        name: String,
        profile: ProfileRef,
        icon: String,
        theme: Option<Theme>,
    },
}

/// A pinned tab to add, or a folder: one to make, or one that's here, and what goes in it.
#[derive(Clone, Debug, PartialEq)]
pub enum Add {
    Tab(Tab),
    /// A split view of its tabs not here yet, two or more.
    Split(Split),
    Folder {
        existing: Option<u64>,
        name: String,
        adds: Vec<Add>,
    },
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.new_profiles.is_empty()
            && self.spaces.is_empty()
            && self.favorites.is_empty()
            && self.history.is_empty()
            && self.addresses.is_empty()
            && self.link_rules.is_empty()
            && self.link_otherwise.is_none()
    }

    pub fn counts(&self) -> Counts {
        fn tabs(adds: &[Add]) -> usize {
            adds.iter()
                .map(|add| match add {
                    Add::Tab(_) => 1,
                    Add::Split(split) => split.tabs.len(),
                    Add::Folder { adds, .. } => tabs(adds),
                })
                .sum()
        }
        let made = self
            .spaces
            .iter()
            .filter(|space| matches!(space.into, Target::New { .. }));
        let pinned = self.spaces.iter().map(|space| tabs(&space.pinned)).sum();
        Counts {
            spaces: made.count(),
            pinned,
            today: self.spaces.iter().map(|space| space.today.len()).sum(),
            favorites: self.favorites.len(),
            pages: self.history.iter().map(|(_, pages)| pages.len()).sum(),
            addresses: (self.addresses.iter()).map(|(_, each)| each.len()).sum(),
            link_rules: self.link_rules.len() + usize::from(self.link_otherwise.is_some()),
        }
    }
}

/// What importing `found` adds to `existing`. `names` are the browser's profiles' names by
/// folder, from its `Local State`. Its default profile is the one this browser started with, and
/// each other joins the profile here with its name, or a new one.
pub fn plan(found: &Found, names: &[(String, String)], existing: &Existing) -> Plan {
    let mut plan = Plan::default();
    let start = existing.profiles.first().map_or(0, |(id, _)| *id);
    let resolve = |profile: &FromProfile, plan: &mut Plan| -> ProfileRef {
        let FromProfile::Custom(dir) = profile else {
            return ProfileRef::Existing(start);
        };
        let name = names
            .iter()
            .find(|(each, _)| each == dir)
            .map_or(dir.as_str(), |(_, name)| name);
        if let Some((id, _)) = existing.profiles.iter().find(|(_, each)| each == name) {
            return ProfileRef::Existing(*id);
        }
        let at = plan
            .new_profiles
            .iter()
            .position(|each| each == name)
            .unwrap_or_else(|| {
                plan.new_profiles.push(name.into());
                plan.new_profiles.len() - 1
            });
        ProfileRef::New(at)
    };
    // Where each found Space goes, for the link rules to it.
    let mut went = Vec::new();
    for space in &found.spaces {
        let profile = resolve(&space.profile, &mut plan);
        let here = existing
            .spaces
            .iter()
            .find(|each| each.name == space.name && profile == ProfileRef::Existing(each.profile));
        // Two Spaces of one name and profile become one here, as a rerun would merge them.
        let planned = plan.spaces.iter().position(|each| match &each.into {
            Target::New {
                name,
                profile: each,
                ..
            } => *name == space.name && *each == profile,
            Target::Existing(id) => here.is_some_and(|here| here.id == *id),
        });
        let rows = here.map_or(&[][..], |here| &here.pinned[..]);
        let mut adds = Vec::new();
        add(&space.pinned, rows, &mut adds);
        let mut today: Vec<Tab> = Vec::new();
        for tab in &space.today {
            let url = tab.url.as_str();
            let here = here.is_some_and(|here| here.today.iter().any(|each| each == url))
                || has(rows, url);
            let planned = planned.is_some_and(|at| {
                let planned = &plan.spaces[at];
                planned.today.iter().any(|each| each.url == url) || planning(&planned.pinned, url)
            });
            let twice = planning(&adds, url) || today.iter().any(|each| each.url == url);
            if !here && !planned && !twice {
                today.push(tab.clone());
            }
        }
        // Where the found Space goes, for the link rules to it: a Space here that gains Today tabs
        // alone is one the plan adds to too.
        went.push(match (planned, here) {
            (Some(at), _) => SpaceRef::Planned(at),
            (None, Some(here)) if adds.is_empty() && today.is_empty() => {
                SpaceRef::Existing(here.id)
            }
            (None, _) => SpaceRef::Planned(plan.spaces.len()),
        });
        match planned {
            Some(at) => {
                merge(&mut plan.spaces[at].pinned, adds);
                plan.spaces[at].today.extend(today);
            }
            None if here.is_some() && adds.is_empty() && today.is_empty() => {}
            None => plan.spaces.push(SpacePlan {
                into: match here {
                    Some(here) => Target::Existing(here.id),
                    None => Target::New {
                        name: space.name.clone(),
                        profile,
                        icon: space.icon.clone(),
                        theme: space.theme,
                    },
                },
                pinned: adds,
                today,
            }),
        }
    }
    for (profile, tabs) in &found.favorites {
        let profile = resolve(profile, &mut plan);
        for tab in tabs {
            let here = existing
                .favorites
                .iter()
                .any(|(each, url)| ProfileRef::Existing(*each) == profile && *url == tab.url);
            let planned = plan
                .favorites
                .iter()
                .any(|(each, planned)| *each == profile && planned.url == tab.url);
            if !here && !planned {
                plan.favorites.push((profile, tab.clone()));
            }
        }
    }
    for (profile, pages) in &found.history {
        let profile = resolve(profile, &mut plan);
        plan.history.push((profile, pages.clone()));
    }
    for (profile, addresses) in &found.addresses {
        let profile = resolve(profile, &mut plan);
        plan.addresses.push((profile, addresses.clone()));
    }
    for rule in &found.link_rules {
        let Some(space) = place(rule.to, &went, &plan.spaces) else {
            continue;
        };
        let same =
            |condition: Condition, text: &str| condition == rule.condition && text == rule.text;
        let here = (existing.link_rules.iter())
            .any(|each| Some(each.space) == id(space) && same(each.condition, &each.text));
        let planned = (plan.link_rules.iter())
            .any(|each| each.space == space && same(each.condition, &each.text));
        if !here && !planned {
            plan.link_rules.push(PlannedRule {
                condition: rule.condition,
                text: rule.text.clone(),
                space,
            });
        }
    }
    // Only over ours' default, so a Space the user picked for everything else stays theirs.
    if existing.link_otherwise == LITTLE {
        plan.link_otherwise = (found.link_otherwise)
            .and_then(|to| place(to, &went, &plan.spaces))
            .filter(|place| *place != SpaceRef::Little);
    }
    plan
}

/// Where `to` goes, by `went`, where each found Space goes among the plan's `spaces`: a Space the
/// plan only adds to is here already, and may have the rule. None for a found Space that doesn't
/// come.
fn place(to: Destination, went: &[SpaceRef], spaces: &[SpacePlan]) -> Option<SpaceRef> {
    Some(match to {
        Destination::Little => SpaceRef::Little,
        Destination::Front => SpaceRef::Front,
        Destination::Space(at) => match *went.get(at)? {
            SpaceRef::Planned(at) => match spaces[at].into {
                Target::Existing(id) => SpaceRef::Existing(id),
                Target::New { .. } => SpaceRef::Planned(at),
            },
            here => here,
        },
    })
}

/// The id a rule to `place` has here, as `link_rules::Rule` holds it; none for a Space the plan
/// makes.
fn id(place: SpaceRef) -> Option<u64> {
    match place {
        SpaceRef::Existing(id) => Some(id),
        SpaceRef::Planned(_) => None,
        SpaceRef::Little => Some(LITTLE),
        SpaceRef::Front => Some(FRONT),
    }
}

/// Adds to `adds` what of `nodes` isn't among `rows`, the rows already in that place.
fn add(nodes: &[Node], rows: &[Row], adds: &mut Vec<Add>) {
    for node in nodes {
        match node {
            Node::Tab(tab) => {
                let here = rows
                    .iter()
                    .any(|row| matches!(row, Row::Tab(url) if *url == tab.url));
                if !here && !lists(adds, &tab.url) {
                    adds.push(Add::Tab(tab.clone()));
                }
            }
            // Its tabs that aren't in this place yet make it, as a tab's would come; one alone
            // comes as a tab.
            Node::Split(split) => {
                let new = |tab: &&Tab| {
                    !rows
                        .iter()
                        .any(|row| matches!(row, Row::Tab(url) if *url == tab.url))
                        && !lists(adds, &tab.url)
                };
                let tabs: Vec<Tab> = split.tabs.iter().filter(new).cloned().collect();
                match &tabs[..] {
                    [] => {}
                    [tab] => adds.push(Add::Tab(tab.clone())),
                    _ => adds.push(Add::Split(Split {
                        tabs,
                        ..split.clone()
                    })),
                }
            }
            Node::Folder { name, children } => {
                let here = rows.iter().find_map(|row| match row {
                    Row::Folder {
                        id,
                        name: each,
                        rows,
                    } if each == name => Some((*id, &rows[..])),
                    _ => None,
                });
                let mut inner = Vec::new();
                add(children, here.map_or(&[], |(_, rows)| rows), &mut inner);
                let planned = adds.iter_mut().find_map(|each| match each {
                    Add::Folder {
                        name: each, adds, ..
                    } if each == name => Some(adds),
                    _ => None,
                });
                match (planned, here) {
                    (Some(planned), _) => merge(planned, inner),
                    // A folder that's here and gets nothing new is left as it is.
                    (None, Some(_)) if inner.is_empty() => {}
                    (None, here) => adds.push(Add::Folder {
                        existing: here.map(|(id, _)| id),
                        name: name.clone(),
                        adds: inner,
                    }),
                }
            }
        }
    }
}

/// Whether a tab at `url` is among `rows`, in a folder or not.
fn has(rows: &[Row], url: &str) -> bool {
    rows.iter().any(|row| match row {
        Row::Tab(each) => each == url,
        Row::Folder { rows, .. } => has(rows, url),
    })
}

/// Whether a tab at `url` is among `adds`, in a folder or not.
fn planning(adds: &[Add], url: &str) -> bool {
    adds.iter().any(|add| match add {
        Add::Tab(tab) => tab.url == url,
        Add::Split(split) => split.tabs.iter().any(|tab| tab.url == url),
        Add::Folder { adds, .. } => planning(adds, url),
    })
}

/// Whether `adds` has a tab at `url` in its own place, not in a folder: a tab's or a split's.
fn lists(adds: &[Add], url: &str) -> bool {
    adds.iter().any(|add| match add {
        Add::Tab(tab) => tab.url == url,
        Add::Split(split) => split.tabs.iter().any(|tab| tab.url == url),
        Add::Folder { .. } => false,
    })
}

/// Puts `more` into `adds`, skipping what's there already.
fn merge(adds: &mut Vec<Add>, more: Vec<Add>) {
    for each in more {
        match each {
            Add::Tab(tab) => {
                if !lists(adds, &tab.url) {
                    adds.push(Add::Tab(tab));
                }
            }
            Add::Split(split) => {
                if !split.tabs.iter().any(|tab| lists(adds, &tab.url)) {
                    adds.push(Add::Split(split));
                }
            }
            Add::Folder {
                existing,
                name,
                adds: inner,
            } => {
                let same = adds.iter_mut().find_map(|add| match add {
                    Add::Folder {
                        name: each, adds, ..
                    } if *each == name => Some(adds),
                    _ => None,
                });
                match same {
                    Some(same) => merge(same, inner),
                    None => adds.push(Add::Folder {
                        existing,
                        name,
                        adds: inner,
                    }),
                }
            }
        }
    }
}
