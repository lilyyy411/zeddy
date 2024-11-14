use std::collections::HashMap;

use anyhow::{anyhow, Result as Res};
use log::info;

use crate::color::palette::ResolvedPalette;
use crate::color::Color;
use crate::schema::json::{JsonTheme, Player, StyleEntry, Syntax, ThemeFamily as JsonThemeFamily};
use crate::schema::kdl::{Action, Modifier, ModifierPath, ThemeFamily};

pub fn generate_json(family: ThemeFamily) -> Res<JsonThemeFamily> {
    info!("Generating JSON file from KDL");

    let ThemeFamily {
        meta,
        palette,
        mut themes,
        common,
    } = family;
    let resolved = palette.into_palette().resolve()?;
    let mut base_theme_file = JsonThemeFamily {
        schema: "https://zed.dev/schema/themes/v0.1.0.json".to_owned(),
        meta,
        themes: Vec::with_capacity(themes.len()),
    };
    // merge all themes with the `common` theme if it exists
    if let Some(common) = common {
        themes.iter_mut().for_each(|x| x.merge(&common));
    }
    let process = |v: Option<Color>| v.map(|x| resolved.lookup(&x)).transpose();
    for theme in themes {
        let mut players = Vec::with_capacity(theme.players.len());
        for player in theme.players {
            players.push(Player {
                cursor: process(player.cursor)?,
                selection: process(player.selection)?,
                background: process(player.background)?,
            });
        }

        let mut base_json_theme = JsonTheme {
            name: theme.name,
            style: HashMap::from_iter([
                ("players".to_owned(), StyleEntry::Players(players)),
                ("syntax".to_owned(), StyleEntry::Syntax(HashMap::default())),
            ]),
            appearance: theme.appearance,
        };
        for Modifier { action, apply } in theme.modifiers {
            for target in apply {
                apply_action(&mut base_json_theme, &action, &resolved, &target)?;
            }
        }
        base_theme_file.themes.push(base_json_theme);
    }
    Ok(base_theme_file)
}

fn apply_action(
    base: &mut JsonTheme,
    action: &Action,
    palette: &ResolvedPalette,
    to: &ModifierPath,
) -> Res<()> {
    match to {
        ModifierPath::Style(path) => {
            if path.starts_with("player") {
                return Err(anyhow!("`style.player` cannot be modified with modifiers. Use the `theme.players` list instead."));
            }
            // Can only apply `color` to `style` items.
            if let Some(color) = &action.color {
                let resolved = palette.lookup(color)?;
                base.style
                    .insert(path.to_owned(), StyleEntry::Normal(Some(resolved)));
            }
            Ok(())
        }
        ModifierPath::Syntax(tail) => {
            process_syntax_path(action, palette, base, tail)?;
            Ok(())
        }
    }
}

fn process_syntax_path(
    action: &Action,
    palette: &ResolvedPalette,
    base: &mut JsonTheme,
    path: &String,
) -> Res<()> {
    let StyleEntry::Syntax(syntax_map) = base.style.get_mut("syntax").unwrap() else {
        return Err(anyhow!("Could not get syntax map"));
    };
    let syntax_entry = syntax_map.entry(path.to_owned()).or_insert_with(|| Syntax {
        color: None,
        background: None,
        font_weight: None,
        font_style: None,
    });
    if let Some(color) = &action.color {
        syntax_entry.color = Some(palette.lookup(color)?);
    }
    if let Some(color) = &action.background {
        syntax_entry.background = Some(palette.lookup(color)?);
    }
    syntax_entry.font_style.clone_from(&action.font_style);
    syntax_entry.font_weight = action.font_weight;
    Ok(())
}
