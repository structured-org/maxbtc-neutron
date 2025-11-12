use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub in_denom: String,
    pub out_denom: String,
}

pub const CONFIG: Item<Config> = Item::new("config");
