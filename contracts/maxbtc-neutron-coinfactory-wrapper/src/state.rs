use cw_storage_plus::Item;
use maxbtc_base::msg::token::DenomMetadata;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub allowed_denom: String,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const DENOM: Item<String> = Item::new("denom");
pub const TOKEN_METADATA: Item<DenomMetadata> = Item::new("denom_metadata");
