use std::{
    collections::{BTreeMap, HashMap, HashSet},
    hash::{Hash, RandomState},
    path::Path,
};

use knus::{
    errors::DecodeError,
    traits::{DecodePartial, ErrorSpan},
    Decode, DecodeScalar,
};

use crate::{color::palette::RawPalette, color::Color, util::ToAnyhow};

use super::{Appearance, Meta};

#[derive(Clone, Debug, Decode)]
pub struct ThemeFamily {
    #[knus(child)]
    pub meta: Meta,
    #[knus(child)]
    pub palette: RawPalette,
    #[knus(children(name = "theme"))]
    pub themes: Vec<Theme>,
    #[knus(child)]
    pub common: Option<Theme>,
}

#[derive(Clone, Debug, Decode)]
pub struct Theme {
    #[knus(child, unwrap(argument))]
    pub name: String,
    #[knus(child, unwrap(argument))]
    pub appearance: Appearance,
    #[knus(children(name = "player"))]
    pub players: Vec<Player>,
    #[knus(children(name = "modifier"))]
    pub modifiers: Vec<Modifier>,
}

impl Theme {
    pub fn merge(&mut self, bottom: &Self) {
        let prev_mod = std::mem::take(&mut self.modifiers);
        let prev_players = std::mem::take(&mut self.players);
        // modifiers that come before are applied first, and then later ones override the previous ones
        self.modifiers.extend_from_slice(&bottom.modifiers);
        self.modifiers.extend_from_slice(&prev_mod);
        self.players.extend_from_slice(&bottom.players);
        self.players.extend_from_slice(&prev_players);
    }

    fn discard_intersection(
        &mut self,
        players: &[Player],
        modifiers: &HashMap<Action, HashSet<ModifierPath>>,
    ) {
        self.modifiers
            .iter_mut()
            .filter_map(|modifier| modifiers.get(&modifier.action).map(|x| (modifier, x)))
            .for_each(|(modifier, intersection)| {
                modifier.apply.retain(|x| !intersection.contains(x));
            });
        self.modifiers.retain(|x| !x.apply.is_empty());
        self.players.retain(|x| !players.contains(x));
    }

    pub fn extract_common(&mut self, other: &mut Self) -> Self {
        let player_intersect = self
            .players
            .iter()
            .filter(|x| other.players.contains(x))
            .cloned()
            .collect::<Vec<_>>();

        let this_modifiers: HashMap<_, _, RandomState> = self
            .modifiers
            .iter()
            .map(|x| (&x.action, &x.apply))
            .collect();

        let other_modifiers: HashMap<_, _, RandomState> = other
            .modifiers
            .iter()
            .map(|x| (&x.action, &x.apply))
            .collect();

        let intersection = this_modifiers
            .iter()
            .filter_map(|(action, &modifiers)| {
                other_modifiers
                    .get(action)
                    .map(|&x| (Action::clone(*action), (modifiers, x)))
            })
            .map(|(action, (this, other))| {
                (
                    action,
                    this.iter()
                        .filter(|x| other.contains(x))
                        .cloned()
                        .collect::<HashSet<_>>(),
                )
            })
            .filter(|(_, x)| !x.is_empty())
            .collect::<HashMap<_, _>>();
        self.discard_intersection(&player_intersect, &intersection);
        other.discard_intersection(&player_intersect, &intersection);
        Theme {
            name: "common".to_owned(),
            appearance: Appearance::Dark,
            players: player_intersect,
            modifiers: intersection
                .into_iter()
                .map(|(action, path)| Modifier {
                    action,
                    apply: <_>::from_iter(path),
                })
                .collect(),
        }
    }
}
#[derive(Clone, Debug, Decode, PartialEq)]
pub struct Player {
    #[knus(child)]
    pub cursor: Option<Color>,
    #[knus(child)]
    pub background: Option<Color>,
    #[knus(child)]
    pub selection: Option<Color>,
}

#[derive(Clone, Debug, Decode)]
pub struct Modifier {
    #[knus(child, unwrap(children))]
    pub apply: Vec<ModifierPath>,
    #[knus(flatten(child))]
    pub action: Action,
}

impl ThemeFamily {
    pub fn read(path: impl AsRef<Path>) -> anyhow::Result<ThemeFamily> {
        let p = path.as_ref();
        let path_name = p.display().to_string();
        let content = std::fs::read_to_string(p)?;
        knus::parse::<ThemeFamily>(&path_name, &content).to_anyhow()
    }
}

#[derive(Clone, Debug, Decode, Hash, PartialEq, Eq)]
pub enum ModifierPath {
    Style(#[knus(argument)] String),
    Syntax(#[knus(argument)] String),
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum BorrowedModifierPath<'a> {
    Style(&'a str),
    Syntax(&'a str),
}

impl BorrowedModifierPath<'_> {
    pub fn into_owned(self) -> ModifierPath {
        match self {
            Self::Style(data) => ModifierPath::Style(data.to_owned()),
            Self::Syntax(data) => ModifierPath::Syntax(data.to_owned()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Palette {
    pub map: BTreeMap<String, String>,
}

impl<S: ErrorSpan> DecodePartial<S> for Palette {
    fn insert_child(
        &mut self,
        node: &knus::ast::SpannedNode<S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<bool, knus::errors::DecodeError<S>> {
        let args = &node.arguments;
        if args.is_empty() {
            return Err(DecodeError::missing(node, "Missing color string"));
        }

        if args.len() != 1 {
            return Err(DecodeError::unexpected(
                &args[1].literal,
                "additional argument",
                "Expected a single color string",
            ));
        }
        let v = String::decode(&args[0], ctx)?;
        if self.map.insert((**node.node_name).to_owned(), v).is_some() {
            return Err(DecodeError::unexpected(
                node,
                "entry",
                "Duplicate palette entry found",
            ));
        }
        Ok(true)
    }
    fn insert_property(
        &mut self,
        _name: &knus::span::Spanned<Box<str>, S>,
        _value: &knus::ast::Value<S>,
        _ctx: &mut knus::decode::Context<S>,
    ) -> Result<bool, knus::errors::DecodeError<S>> {
        Ok(false)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Decode, Eq, Hash)]
pub struct Action {
    #[knus(child)]
    pub color: Option<Color>,
    #[knus(child)]
    pub background: Option<Color>,
    #[knus(child, unwrap(argument))]
    pub font_weight: Option<u16>,
    #[knus(child, unwrap(argument))]
    pub font_style: Option<String>,
}
