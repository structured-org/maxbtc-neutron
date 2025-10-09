use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;

#[cw_serde]
pub struct Config {
    pub locker: Addr,
    pub unlocker: Addr,
    pub withdraw_manager_contract: Addr,
    pub asset: String,
}

/// A single global config item
pub const CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub enum State {
    Locked { amount: Uint128, at_timestamp: u64 },
    Unlocked {},
}

pub const STATE: Item<State> = Item::new("state");
