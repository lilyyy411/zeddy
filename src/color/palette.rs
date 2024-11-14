use std::{collections::HashMap, fmt::Debug};

use crate::color::{BaseColorKind, Color, ColorModifiers, HexColor};
use anyhow::anyhow;
use bimap::BiMap;
use colornamer::{ColorNamer, Colors};
use knus::Decode;

/// The raw, unsanitized palette input directly from the theme file.
/// This then needs to converted to a `Palette`.
#[derive(Debug, Clone, Decode, Default)]
pub struct RawPalette {
    #[knus(children)]
    pub(crate) colors: Vec<ColorNode>,
}

impl RawPalette {
    pub fn into_palette(self) -> Palette {
        Palette {
            colors: self.colors.into_iter().map(ColorNode::into_tuple).collect(),
        }
    }
}

#[derive(Debug, Clone, Decode)]
pub struct ColorNode {
    #[knus(node_name)]
    pub name: String,
    #[knus(argument)]
    pub base: BaseColorKind,
    #[knus(flatten(property))]
    pub modifiers: ColorModifiers,
}
impl ColorNode {
    pub fn into_tuple(self) -> (String, Color) {
        let ColorNode {
            name,
            base,
            modifiers,
        } = self;
        (name, Color { base, modifiers })
    }
}

/// The raw, unsanitized, unresolved input from the theme file, but as a mapping instead of a sequence.
/// Has information about both modifiers and color names.
pub struct Palette {
    pub colors: HashMap<String, Color>,
}

impl Palette {
    fn resolve_color<'a>(
        &'a self,
        name: &'a str,
        color: &'a Color,
        partial_resolutions: &mut HashMap<String, HexColor>,
        deps: &mut Vec<&'a str>,
    ) -> anyhow::Result<HexColor> {
        if let Some(color) = partial_resolutions.get(name) {
            // We already resolved this color
            return Ok(*color);
        }
        // TODO: use a better data structure
        if let Some(idx) = deps
            .iter()
            .enumerate()
            .find_map(|x| (*x.1 == name).then_some(x.0))
        {
            let deps = &deps[idx..];
            if deps.len() <= 1 {
                return Err(anyhow!(
                    "cyclic dependency in palette: {name} directly depends on itself!"
                ));
            }
            let mut iter = deps.iter();
            let mut msg = String::with_capacity(1024)
                + &format!(
                    "cyclic dependency in palette:\n    {} depends on {}",
                    iter.next().unwrap(),
                    iter.next().unwrap()
                );

            for &i in iter {
                msg += "\n        which depends on ";
                msg += i;
            }
            msg += "\n        which depends on ";
            msg += name;

            return Err(anyhow!(msg));
        }
        deps.push(name);
        let resolved = match color.base {
            BaseColorKind::Hex(hex) => hex,
            BaseColorKind::PaletteReference(ref reference) => {
                let Some(dep_color) = self.colors.get(reference) else {
                    return Err(anyhow!("could not find color {reference} in the palette"));
                };
                self.resolve_color(reference, dep_color, partial_resolutions, deps)?
            }
        };
        let modified = resolved.apply_modifiers(color.modifiers);
        partial_resolutions.insert(name.to_owned(), modified);
        Ok(modified)
    }
    pub fn resolve(self) -> anyhow::Result<ResolvedPalette> {
        let mut resolutions = HashMap::with_capacity(self.colors.len());
        let mut deps = Vec::with_capacity(self.colors.len());
        for (name, color) in &self.colors {
            self.resolve_color(name, color, &mut resolutions, &mut deps)?;
            deps.clear();
        }
        Ok(ResolvedPalette {
            colors: resolutions,
        })
    }
}

/// The final resolved palette of colors.
#[derive(Debug, Clone)]
pub struct ResolvedPalette {
    pub colors: HashMap<String, HexColor>,
}
impl ResolvedPalette {
    pub fn into_raw_palette(self) -> RawPalette {
        let mut colors = self
            .colors
            .into_iter()
            .map(|(name, color)| ColorNode {
                name,
                base: BaseColorKind::Hex(color),
                modifiers: <_>::default(),
            })
            .collect::<Vec<_>>();
        // we have to do it like this or else we get a lifetime error
        colors.sort_unstable_by(|x, y| x.name.cmp(&y.name));
        RawPalette { colors }
    }
    pub fn lookup(&self, color: &Color) -> anyhow::Result<HexColor> {
        let hex = match color.base {
            BaseColorKind::Hex(hex) => hex,
            BaseColorKind::PaletteReference(ref pal_ref) => *self
                .colors
                .get(pal_ref)
                .ok_or_else(|| anyhow!("could not find color {pal_ref:?} in the palette"))?,
        };
        Ok(hex.apply_modifiers(color.modifiers))
    }
}
fn alpha_to_modifier(alpha: u8) -> f32 {
    f32::from(alpha) / 255.0
}
/// Generates a palette based on input colors, attempting to simplify repeated and similar colors, and assigning colors names
pub struct PaletteGenerator {
    rgb_to_name: BiMap<[u8; 3], String>,
    namer: ColorNamer,
}
impl Default for PaletteGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for PaletteGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaletteGenerator")
            .field("rgb_to_names", &self.rgb_to_name)
            .finish_non_exhaustive()
    }
}
impl PaletteGenerator {
    pub fn new() -> Self {
        Self {
            rgb_to_name: <_>::default(),
            namer: ColorNamer::new(Colors::all()),
        }
    }

    /// Feeds a single color into the generator
    pub fn feed(&mut self, color: HexColor) {
        let HexColor([r, g, b, _]) = color;
        let rgb = [r, g, b];
        if self.rgb_to_name.contains_left(&rgb) {
            return;
        }
        // This api is so bad... why do I need a hex string to name the damn color?
        // I should probably fork the colornamer crate one day...
        // You don't understand how bad their hex parser implementation is.
        let name = self
            .namer
            .name_hex_color(&format!("#{r:02x}{g:02x}{b:02x}"))
            .unwrap() // can only error on invalid hex
            .to_lowercase()
            .trim()
            .replace(' ', "-");
        let mut name2 = name.clone();
        let mut idx = 1;
        // todo: make this more efficient. this is extremely bad because of constant
        // allocations
        while let Some(&right) = self.rgb_to_name.get_by_right(&name2) {
            if right == rgb {
                break;
            }
            name2 = format!("{name}-{idx}");
            idx += 1;
        }
        self.rgb_to_name.insert(rgb, name2);
    }

    pub fn lookup(&self, color: HexColor) -> Color {
        let HexColor([r, g, b, a]) = color;
        let rgb = [r, g, b];
        if let Some(name) = self.rgb_to_name.get_by_left(&rgb) {
            let base = BaseColorKind::PaletteReference(name.clone());
            let alpha = (a != 255).then(|| alpha_to_modifier(a));
            Color {
                base,
                modifiers: ColorModifiers {
                    alpha,
                    ..<_>::default()
                },
            }
        } else {
            Color {
                base: BaseColorKind::Hex(color),
                ..<_>::default()
            }
        }
    }
    pub fn into_resolved_palette(self) -> ResolvedPalette {
        ResolvedPalette {
            colors: self
                .rgb_to_name
                .into_iter()
                .map(|([r, g, b], name)| (name, HexColor([r, g, b, 255])))
                .collect(),
        }
    }
}
