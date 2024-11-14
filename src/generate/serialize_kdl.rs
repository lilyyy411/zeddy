//! A flawed implementation of KDL serialization that only works in our specific cases.
//! I had to do all this because there is no good serde implementation for KDL (that isn't wildly broken)

use std::{fmt::Display, io::Write};

use log::debug;

use crate::{
    color::{palette::RawPalette, BaseColorKind, Color},
    schema::kdl::{Action, Modifier, ModifierPath, Player, Theme, ThemeFamily},
    schema::{Appearance, Meta},
};

pub struct KdlSerializer<W: Write> {
    indent: usize,
    writer: W,
}
impl<W: Write> KdlSerializer<W> {
    pub fn new(writer: W) -> Self {
        Self { indent: 0, writer }
    }
    fn indent(&mut self) {
        self.indent += 1;
    }
    fn dedent(&mut self) {
        self.indent -= 1;
    }
    fn write_indent(&mut self) -> std::io::Result<()> {
        for _ in 0..self.indent {
            self.writer.write_all("    ".as_bytes())?;
        }
        Ok(())
    }
    pub fn children_block(
        &mut self,
        node_name: impl Display,
    ) -> std::io::Result<ChildrenBlock<'_, W>> {
        self.writer.write_all(b"\n")?;
        self.write_indent()?;
        self.writer.write_fmt(format_args!("{node_name} {{"))?;
        Ok(ChildrenBlock { inner: self })
    }

    pub fn inline_node(&mut self, node_name: impl Display) -> std::io::Result<InlineNode<'_, W>> {
        self.writer.write_all(b"\n")?;
        self.write_indent()?;
        self.writer.write_fmt(format_args!("{node_name}"))?;
        Ok(InlineNode { inner: self })
    }
}

pub struct ChildrenBlock<'a, W: Write> {
    inner: &'a mut KdlSerializer<W>,
}

impl<'a, W: Write> ChildrenBlock<'a, W> {
    pub fn child(self, name: impl Display, child: impl SerializeKdl) -> std::io::Result<Self> {
        self.inner.indent();
        child.serialize(name, self.inner)?;
        self.inner.dedent();
        Ok(self)
    }
    pub fn children(
        mut self,
        children: impl IntoIterator<Item = (impl Display, impl SerializeKdl)>,
    ) -> std::io::Result<Self> {
        for (name, child) in children {
            self = self.child(name, child)?;
        }
        Ok(self)
    }
    pub fn finish(self) -> std::io::Result<&'a mut KdlSerializer<W>> {
        self.inner.writer.write_all(b"\n")?;
        self.inner.write_indent()?;
        self.inner.writer.write_all(b"}")?;
        Ok(self.inner)
    }
}

pub struct InlineNode<'a, W: Write> {
    inner: &'a mut KdlSerializer<W>,
}

impl<'a, W: Write> InlineNode<'a, W> {
    pub fn arg(self, arg: impl SerializeKdlScalar) -> std::io::Result<Self> {
        self.inner.writer.write_all(b" ")?;
        arg.serialize_scalar(self.inner)?;
        Ok(self)
    }
    pub fn property(
        self,
        prop: impl Display,
        value: Option<impl SerializeKdlScalar>,
    ) -> std::io::Result<Self> {
        if let Some(value) = value {
            self.inner.writer.write_fmt(format_args!(" {prop}="))?;
            value.serialize_scalar(self.inner)?;
        }
        Ok(self)
    }
    #[allow(dead_code)]
    pub fn properties(
        mut self,
        props: impl IntoIterator<Item = (impl Display, Option<impl SerializeKdlScalar>)>,
    ) -> std::io::Result<Self> {
        for (prop, value) in props {
            self = self.property(prop, value)?;
        }
        Ok(self)
    }
    #[allow(clippy::unnecessary_wraps)]
    pub fn finish(self) -> std::io::Result<&'a mut KdlSerializer<W>> {
        self.inner.dedent();
        Ok(self.inner)
    }
}

/// Serialize data to KDL. Note that this does not properly handle all structures right now
/// and is only specific to our custom file format.
pub trait SerializeKdl {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()>;
}

pub trait SerializeKdlScalar {
    fn serialize_scalar<W: Write>(&self, serializer: &mut KdlSerializer<W>) -> std::io::Result<()>;
}

impl SerializeKdlScalar for String {
    fn serialize_scalar<W: Write>(&self, serializer: &mut KdlSerializer<W>) -> std::io::Result<()> {
        serializer.writer.write_fmt(format_args!("{self:?}"))
    }
}

impl SerializeKdlScalar for &'_ str {
    fn serialize_scalar<W: Write>(&self, serializer: &mut KdlSerializer<W>) -> std::io::Result<()> {
        serializer.writer.write_fmt(format_args!("{self:?}"))
    }
}

impl SerializeKdl for String {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        serializer.inline_node(node_name)?.arg(self)?.finish()?;
        Ok(())
    }
}

impl<T: SerializeKdlScalar> SerializeKdlScalar for &'_ T {
    fn serialize_scalar<W: Write>(&self, serializer: &mut KdlSerializer<W>) -> std::io::Result<()> {
        SerializeKdlScalar::serialize_scalar(*self, serializer)
    }
}

impl<T: SerializeKdl> SerializeKdl for &'_ T {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        SerializeKdl::serialize(*self, node_name, serializer)
    }
}
impl<T: SerializeKdlScalar> SerializeKdlScalar for Option<T> {
    fn serialize_scalar<W: Write>(&self, serializer: &mut KdlSerializer<W>) -> std::io::Result<()> {
        if let Some(scalar) = self {
            scalar.serialize_scalar(serializer)
        } else {
            Ok(())
        }
    }
}

impl SerializeKdlScalar for f32 {
    fn serialize_scalar<W: Write>(&self, serializer: &mut KdlSerializer<W>) -> std::io::Result<()> {
        serializer.writer.write_fmt(format_args!("{self:?}"))
    }
}
impl SerializeKdlScalar for BaseColorKind {
    fn serialize_scalar<W: Write>(&self, serializer: &mut KdlSerializer<W>) -> std::io::Result<()> {
        match self {
            Self::Hex(color) => color.to_string().serialize_scalar(serializer),
            Self::PaletteReference(reference) => reference.serialize_scalar(serializer),
        }
    }
}
impl<T: SerializeKdl> SerializeKdl for Vec<T> {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        for item in self {
            item.serialize(&node_name, serializer)?;
        }
        Ok(())
    }
}
impl<T: SerializeKdl> SerializeKdl for Option<T> {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        if let Some(this) = self {
            this.serialize(node_name, serializer)?;
        }
        Ok(())
    }
}
impl SerializeKdlScalar for u16 {
    fn serialize_scalar<W: Write>(&self, serializer: &mut KdlSerializer<W>) -> std::io::Result<()> {
        serializer.writer.write_fmt(format_args!("{self:?}"))
    }
}
impl SerializeKdl for ThemeFamily {
    fn serialize<W: Write>(
        &self,
        _node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        self.meta.serialize("meta", serializer)?;
        self.palette.serialize("palette", serializer)?;
        self.common.serialize("common", serializer)?;
        self.themes.serialize("theme", serializer)?;

        Ok(())
    }
}

impl SerializeKdl for RawPalette {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        serializer
            .children_block(node_name)?
            .children(self.colors.iter().map(|node| node.clone().into_tuple()))?
            .finish()?;
        Ok(())
    }
}

impl SerializeKdl for Color {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        serializer
            .inline_node(node_name)?
            .arg(&self.base)?
            .property("alpha", self.modifiers.alpha)?
            .property("lighten", self.modifiers.lighten)?
            .property("darken", self.modifiers.darken)?
            .property("saturate", self.modifiers.saturate)?
            .property("desaturate", self.modifiers.desaturate)?
            .property("hue-shift", self.modifiers.hue_shift)?
            .finish()?;
        Ok(())
    }
}

impl SerializeKdl for Meta {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        serializer
            .children_block(node_name)?
            .child("name", &self.name)?
            .child("author", &self.author)?
            .finish()?;
        Ok(())
    }
}

impl SerializeKdl for Theme {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        serializer
            .children_block(node_name)?
            .child("name", &self.name)?
            .child("appearance", &self.appearance)?
            .child("modifier", &self.modifiers)?
            .child("player", &self.players)?
            .finish()?;
        Ok(())
    }
}
impl SerializeKdlScalar for Appearance {
    fn serialize_scalar<W: Write>(&self, serializer: &mut KdlSerializer<W>) -> std::io::Result<()> {
        let s = match self {
            Self::Dark => "dark",
            Self::Light => "light",
        };
        s.serialize_scalar(serializer)
    }
}

impl SerializeKdl for Appearance {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        serializer.inline_node(node_name)?.arg(self)?.finish()?;
        Ok(())
    }
}

impl SerializeKdl for Modifier {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        serializer
            .children_block(node_name)?
            .child("action", &self.action)?
            .child("apply", ApplyBlock(&self.apply))?
            .finish()?;
        Ok(())
    }
}
struct ApplyBlock<'a>(&'a [ModifierPath]);
impl SerializeKdl for ApplyBlock<'_> {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        serializer
            .children_block(node_name)?
            .children(self.0.iter().map(|x| ("", x)))?
            .finish()?;
        Ok(())
    }
}

impl SerializeKdl for ModifierPath {
    fn serialize<W: Write>(
        &self,
        _node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        let (head, tail) = match self {
            Self::Style(tail) => ("style", tail),
            Self::Syntax(tail) => ("syntax", tail),
        };
        serializer.inline_node(head)?.arg(tail)?.finish()?;
        Ok(())
    }
}
impl SerializeKdl for Action {
    fn serialize<W: Write>(
        &self,
        _node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        self.color.serialize("color", serializer)?;
        self.background.serialize("background", serializer)?;
        self.font_style.serialize("font-style", serializer)?;
        if let Some(font_weight) = self.font_weight {
            serializer
                .inline_node("font-weight")?
                .arg(font_weight)?
                .finish()?;
        }

        Ok(())
    }
}
impl SerializeKdl for Player {
    fn serialize<W: Write>(
        &self,
        node_name: impl Display,
        serializer: &mut KdlSerializer<W>,
    ) -> std::io::Result<()> {
        serializer
            .children_block(node_name)?
            .child("cursor", &self.cursor)?
            .child("selection", &self.selection)?
            .child("background", &self.background)?
            .finish()?;
        Ok(())
    }
}
pub fn serialize_kdl<W: Write>(writer: W, family: &ThemeFamily) -> std::io::Result<()> {
    debug!("Serializing to KDL");
    family.serialize("", &mut KdlSerializer::new(writer))
}
