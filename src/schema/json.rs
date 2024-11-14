use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    color::HexColor,
    schema::{Appearance, Meta},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct ThemeFamily {
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(flatten)]
    pub meta: Meta,
    pub themes: Vec<JsonTheme>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct JsonTheme {
    pub name: String,
    pub appearance: Appearance,
    pub style: HashMap<String, StyleEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub cursor: Option<HexColor>,
    pub background: Option<HexColor>,
    pub selection: Option<HexColor>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StyleEntry {
    Syntax(HashMap<String, Syntax>),
    Players(Vec<Player>),
    Normal(Option<HexColor>),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Syntax {
    pub color: Option<HexColor>,
    pub background: Option<HexColor>,
    pub font_weight: Option<u16>,
    pub font_style: Option<String>,
}
