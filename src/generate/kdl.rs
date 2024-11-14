use std::collections::HashMap;

use log::{debug, warn};
use multimap::MultiMap;

use crate::{
    color::palette::{PaletteGenerator, RawPalette},
    color::{Color, HexColor},
    schema::json::{StyleEntry, Syntax, ThemeFamily as JsonThemeFamily},
    schema::kdl::{
        Action, BorrowedModifierPath, Modifier, ModifierPath, Player, Theme, ThemeFamily,
    },
};

pub trait StyleVisitor {
    fn visit_syntax(&mut self, _path: BorrowedModifierPath<'_>, _syntax: &Syntax) {}
    fn visit_color(&mut self, _key: Option<BorrowedModifierPath<'_>>, _color: HexColor) {}
    fn visit_font_weight(&mut self, _key: BorrowedModifierPath<'_>, _weight: u16) {}
    fn visit_font_style(&mut self, _key: BorrowedModifierPath<'_>, _style: &str) {}
}

pub fn visit_styles<V: StyleVisitor>(visitor: &mut V, map: &HashMap<String, StyleEntry>) {
    for (key, value) in map {
        match value {
            StyleEntry::Normal(Some(color)) => {
                visitor.visit_color(Some(BorrowedModifierPath::Style(key)), *color);
            }
            StyleEntry::Players(players) => {
                for player in players {
                    if let Some(color) = player.cursor {
                        visitor.visit_color(None, color);
                    }
                    if let Some(color) = player.background {
                        visitor.visit_color(None, color);
                    }
                    if let Some(color) = player.selection {
                        visitor.visit_color(None, color);
                    }
                }
            }
            StyleEntry::Syntax(syntax_map) => {
                for (name, syntax) in syntax_map {
                    let path = BorrowedModifierPath::Syntax(name);
                    visitor.visit_syntax(path, syntax);
                    if let Some(color) = syntax.color {
                        visitor.visit_color(Some(path), color);
                    }
                    if let Some(color) = syntax.background {
                        visitor.visit_color(Some(path), color);
                    }
                    if let Some(style) = &syntax.font_style {
                        visitor.visit_font_style(path, style);
                    }
                    if let Some(weight) = syntax.font_weight {
                        visitor.visit_font_weight(path, weight);
                    }
                }
            }
            StyleEntry::Normal(None) => {}
        }
    }
}

#[derive(Default)]
pub struct ColorVisitor {
    generator: PaletteGenerator,
}
impl ColorVisitor {
    pub fn into_inner(self) -> PaletteGenerator {
        self.generator
    }
}

impl StyleVisitor for ColorVisitor {
    fn visit_color(&mut self, _key: Option<BorrowedModifierPath<'_>>, color: HexColor) {
        self.generator.feed(color);
    }
}

struct ModifierVisitor<'a> {
    colors: MultiMap<Color, ModifierPath>,
    background: MultiMap<Color, ModifierPath>,
    font_weight: MultiMap<u16, ModifierPath>,
    font_style: MultiMap<String, ModifierPath>,
    palette: &'a PaletteGenerator,
}

impl<'a> ModifierVisitor<'a> {
    pub fn new(palette: &'a PaletteGenerator) -> Self {
        Self {
            colors: <_>::default(),
            background: <_>::default(),
            font_weight: <_>::default(),
            font_style: <_>::default(),
            palette,
        }
    }
    pub fn into_modifiers(self) -> Vec<Modifier> {
        let mut modifiers = vec![];
        modifiers.extend(self.colors.into_iter().map(|(color, paths)| Modifier {
            apply: paths.clone(),
            action: Action {
                color: Some(color),
                ..<_>::default()
            },
        }));
        modifiers.extend(self.background.into_iter().map(|(color, paths)| Modifier {
            apply: paths.clone(),
            action: Action {
                background: Some(color),
                ..<_>::default()
            },
        }));
        modifiers.extend(self.font_style.into_iter().map(|(style, paths)| Modifier {
            apply: paths.clone(),
            action: Action {
                font_style: Some(style.clone()),
                ..<_>::default()
            },
        }));
        modifiers.extend(
            self.font_weight
                .into_iter()
                .map(|(weight, paths)| Modifier {
                    apply: paths.clone(),
                    action: Action {
                        font_weight: Some(weight),
                        ..<_>::default()
                    },
                }),
        );
        modifiers
    }
}

impl StyleVisitor for ModifierVisitor<'_> {
    fn visit_color(&mut self, path: Option<BorrowedModifierPath<'_>>, color: HexColor) {
        let Some(path) = path else {
            return;
        };
        let color = self.palette.lookup(color);
        self.colors.insert(color, path.into_owned());
    }
    fn visit_font_style(&mut self, path: BorrowedModifierPath<'_>, style: &str) {
        self.font_style.insert(style.to_owned(), path.into_owned());
    }
    fn visit_font_weight(&mut self, path: BorrowedModifierPath<'_>, weight: u16) {
        self.font_weight.insert(weight, path.into_owned());
    }
}

pub fn generate_kdl(theme_family: JsonThemeFamily) -> ThemeFamily {
    debug!("Converting from JSON to KDL");
    let mut base_theme = ThemeFamily {
        meta: theme_family.meta,
        palette: RawPalette::default(),
        themes: vec![],
        common: None,
    };
    let mut color_visitor = ColorVisitor::default();
    debug!("Generating palettes");
    for theme in &theme_family.themes {
        visit_styles(&mut color_visitor, &theme.style);
    }

    let palette_generator = color_visitor.into_inner();
    debug!("Generated palette {:?}", palette_generator);

    for theme in theme_family.themes {
        debug!("Translating theme {}", theme.name);
        let mut kdl_theme = Theme {
            appearance: theme.appearance.clone(),
            modifiers: vec![],
            players: vec![],
            name: theme.name.clone(),
        };
        let mut modifier_visitor = ModifierVisitor::new(&palette_generator);
        if let Some(StyleEntry::Players(players)) = theme.style.get("players") {
            debug!("Translating players");
            for player in players {
                kdl_theme.players.push(Player {
                    cursor: player.cursor.map(|x| palette_generator.lookup(x)),
                    selection: player.selection.map(|x| palette_generator.lookup(x)),
                    background: player.background.map(|x| palette_generator.lookup(x)),
                });
            }
        }
        debug!("Translating expressions to modifiers");
        visit_styles(&mut modifier_visitor, &theme.style);
        let modifiers = modifier_visitor.into_modifiers();
        debug!("Got modifiers {modifiers:?}");
        kdl_theme.modifiers = modifiers;
        debug!("Finished translating theme {}", kdl_theme.name);
        base_theme.themes.push(kdl_theme);
    }

    base_theme.palette = palette_generator.into_resolved_palette().into_raw_palette();
    if let [t1, t2] = base_theme.themes.as_mut_slice() {
        base_theme.common = Some(t1.extract_common(t2));
    } else if base_theme.themes.len() > 2 {
        warn!("Extracting common attributes from more than 2 themes in a family is not supported yet. A `common` node will not be made.");
    }
    base_theme
}
