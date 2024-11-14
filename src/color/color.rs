use anyhow::anyhow;
use knus::{errors::DecodeError, traits::ErrorSpan, Decode, DecodeScalar};
use palette::{
    DarkenAssign, DesaturateAssign, IntoColor, Lcha, LightenAssign, SaturateAssign, ShiftHueAssign,
    Srgba,
};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{convert::Infallible, fmt::Display, hash::Hash, num::FpCategory, str::FromStr};

/// A color in the custom KDL format.
#[derive(Debug, Clone, Decode, Default, PartialEq, Eq, Hash)]
pub struct Color {
    #[knus(argument)]
    pub base: BaseColorKind,
    #[knus(flatten(property))]
    pub modifiers: ColorModifiers,
}

#[derive(Clone, Copy, Debug, Decode, Default, PartialEq)]
pub struct ColorModifiers {
    #[knus(property)]
    pub alpha: Option<f32>,
    #[knus(property)]
    pub lighten: Option<f32>,
    #[knus(property)]
    pub darken: Option<f32>,
    #[knus(property)]
    pub saturate: Option<f32>,
    #[knus(property)]
    pub desaturate: Option<f32>,
    #[knus(property)]
    pub hue_shift: Option<f32>,
}

// trust me bro
impl Eq for ColorModifiers {}
impl Hash for ColorModifiers {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let mut hash_opt_f32 = |x: Option<f32>| {
            x.map(|x| match x.classify() {
                FpCategory::Zero => 0u32,
                FpCategory::Nan => f32::NAN.to_bits(),
                _ => x.to_bits(),
            })
            .hash(state);
        };
        hash_opt_f32(self.alpha);
        hash_opt_f32(self.lighten);
        hash_opt_f32(self.darken);
        hash_opt_f32(self.saturate);
        hash_opt_f32(self.desaturate);
        hash_opt_f32(self.hue_shift);
    }
}
/// The base type of a color entry before
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum BaseColorKind {
    /// A Reference to a color name in the palette
    PaletteReference(String),
    /// A hex color (#rrggbb(aa))
    Hex(HexColor),
}
impl Default for BaseColorKind {
    fn default() -> Self {
        BaseColorKind::Hex(HexColor([0xB0, 0x0B, 0x13, 0x50]))
    }
}

impl FromStr for BaseColorKind {
    type Err = Infallible;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if let Some(hex) = parse_hex_color(input) {
            Ok(BaseColorKind::Hex(hex))
        } else {
            Ok(Self::PaletteReference(input.to_owned()))
        }
    }
}
impl<S: ErrorSpan> DecodeScalar<S> for BaseColorKind {
    fn decode(
        value: &knus::ast::Value<S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        let s = String::decode(value, ctx)?;
        s.parse()
            .map_err(|x| DecodeError::conversion(&value.literal, x))
    }
    fn raw_decode(
        value: &knus::span::Spanned<knus::ast::Literal, S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        String::raw_decode(value, ctx)?
            .parse()
            .map_err(|x| DecodeError::conversion(value, x))
    }
    fn type_check(
        _: &Option<knus::span::Spanned<knus::ast::TypeName, S>>,
        _: &mut knus::decode::Context<S>,
    ) {
    }
}

/// Parses a hex color in the form of `#rrggbb(aa)` where `aa` is optional.
/// Letters are case insensitive. Returns `None` on invalid inputs.
pub fn parse_hex_color(input: &str) -> Option<HexColor> {
    const QUARTER_HEXY_DEVIL: u64 = 0x6666_0000_0000_0000u64;
    const ZERO: u64 = 0x3030_3030_3030_3030;
    const SIXTEEN: u64 = 0x1010_1010_1010_1010;
    if input.len() != 7 && input.len() != 9 {
        return None;
    }
    let mut ptr = input.as_ptr();
    let mut data: u64 = unsafe {
        if *ptr != b'#' {
            return None;
        }

        ptr = ptr.add(1);
        if input.len() == 9 {
            ptr.cast::<u64>().read_unaligned()
        } else if cfg!(target_endian = "little") {
            u64::from(ptr.cast::<u32>().read_unaligned())
                | u64::from(ptr.add(2).cast::<u32>().read_unaligned()) << 16
                | QUARTER_HEXY_DEVIL
        } else {
            (u64::from(ptr.cast::<u32>().read_unaligned())) << 32
                | u64::from(ptr.add(2).cast::<u32>().read_unaligned()) << 16
                | QUARTER_HEXY_DEVIL.to_le()
        }
    };
    // get the values of the digits (and apologize later if we cannot)
    data = data.wrapping_sub(ZERO);
    // mask the values that are above 9
    let above_nine = (data | data.wrapping_add(ZERO >> 3)) & SIXTEEN;
    // try to lowercase everything that is not a number
    data |= above_nine << 1;
    // get the values of the letters (and apologize later if we cannot)
    data = data.wrapping_sub((above_nine >> 4).wrapping_mul(0x27));
    // set an overflow if there is a value that is less than 9 that wasn't
    // there in the last step (ie, filter off : through ; and [ through `).
    data |= 0x19f9_f9f9_f9f9_f9f9u64.wrapping_sub(data) & above_nine;

    // did we overflow anywhere
    if data & 0xf0f0_f0f0_f0f0_f0f0 != 0 {
        return None;
    }
    data = data.to_le();
    // swap and deinterleave the nybles
    data = (data >> 8) | (data << 4);
    data &= 0x00ff_00ff_00ff_00ff;
    data |= data >> 8;
    data &= 0x0000_ffff_0000_ffff;
    data |= data >> 16;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "despite how it looks, there is no truncation possible"
    )]
    Some(HexColor((data as u32).to_le_bytes()))
}

/// A hex color input
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, DeserializeFromStr, SerializeDisplay)]
pub struct HexColor(pub [u8; 4]);

impl HexColor {
    pub(crate) fn apply_modifiers(self, modifiers: ColorModifiers) -> Self {
        let HexColor([r, g, b, a]) = self;
        let rgba = Srgba::from((r, g, b, a)).into_format();
        let mut lcha: Lcha = rgba.into_color();

        if let Some(alpha) = modifiers.alpha {
            lcha.alpha *= alpha;
        }

        if let Some(multiplier) = modifiers.darken {
            lcha.darken_assign(multiplier);
        }

        if let Some(multiplier) = modifiers.lighten {
            lcha.lighten_assign(multiplier);
        }

        if let Some(multiplier) = modifiers.desaturate {
            lcha.desaturate_assign(multiplier);
        }

        if let Some(multiplier) = modifiers.saturate {
            lcha.saturate_assign(multiplier);
        }

        if let Some(offset) = modifiers.hue_shift {
            lcha.shift_hue_assign(offset);
        }

        let srgba: Srgba = lcha.into_color();
        let rgba = srgba.into_format();

        HexColor([rgba.red, rgba.green, rgba.blue, rgba.alpha])
    }
}

impl Display for HexColor {
    #[allow(
        clippy::many_single_char_names,
        reason = "there is literally no better name"
    )]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self([r, g, b, a]) = self;
        write!(f, "#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

impl<S: ErrorSpan> DecodeScalar<S> for HexColor {
    fn raw_decode(
        value: &knus::span::Spanned<knus::ast::Literal, S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        let s = String::raw_decode(value, ctx)?;
        s.parse().map_err(|e| DecodeError::conversion(value, e))
    }
    fn decode(
        value: &knus::ast::Value<S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        Self::raw_decode(&value.literal, ctx)
    }
    fn type_check(
        _: &Option<knus::span::Spanned<knus::ast::TypeName, S>>,
        _: &mut knus::decode::Context<S>,
    ) {
    }
}

impl FromStr for HexColor {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_hex_color(s).ok_or_else(|| anyhow!("Expected hex color"))
    }
}
