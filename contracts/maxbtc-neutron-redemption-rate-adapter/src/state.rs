use cosmwasm_schema::cw_serde;
use cw_storage_plus::Item;

#[cw_serde]
pub struct Config {
    pub fee_bps: u16,
    pub twaer_provider_contract: String,
    pub denom: String,
}

pub const CONFIG: Item<Config> = Item::new("config");
