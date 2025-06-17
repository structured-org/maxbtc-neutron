use cosmwasm_std::{Addr, Coin, Uint128};
use cw_storage_plus::{Item};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub transfer_denom: String,
    pub to_chain_receiver: String,
    pub to_chain_recover_address: String,
    pub to_chain_source_channel: String,
    pub to_chain_entry_contract_address: String,
    pub to_chain_callback_contract_address: String,
    pub max_fee: Coin,
    pub oracle_address: Addr,
    pub relay_fee: Coin,
}

pub const CONFIG: Item<Config> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TransferInfo {
    pub amount: Uint128,
    pub receiver: String,
}
