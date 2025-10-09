use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::Item;

#[cw_serde]
pub struct Config {
    /// Factory contract that owns all admin privileges
    pub factory_contract: Addr,
    /// The token-factory sub-denom used for the maxBTC token
    pub denom: String,
}

/// A single global config item
pub const CONFIG: Item<Config> = Item::new("config");
