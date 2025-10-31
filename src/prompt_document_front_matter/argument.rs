use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Argument {
    pub date: String,
    pub description: String,
    pub required: bool,
    pub title: String,
}
