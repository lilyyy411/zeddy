pub mod json;
pub mod kdl;
pub use json::ThemeFamily as JsonThemeFamily;
pub use kdl::ThemeFamily as KdlThemeFamily;

use knus::{Decode, DecodeScalar};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, DecodeScalar, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    Light,
    Dark,
}

#[derive(Clone, Debug, Decode, Deserialize, Serialize)]
pub struct Meta {
    #[knus(child, unwrap(argument))]
    pub name: String,
    #[knus(child, unwrap(argument))]
    pub author: String,
}
