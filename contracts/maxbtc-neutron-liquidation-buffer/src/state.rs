use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;
use cw_storage_plus::Item;

#[cw_serde]
pub struct Config {
    // TODO: just for testing purposes
    pub owned_maxbtc: Uint128,
    // TODO: just for testing purposes
    pub owned_btc: Uint128,
}

/// A single global config item
pub const CONFIG: Item<Config> = Item::new("config");
