use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Timestamp};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub core_contract: Addr,
    pub period_timeout: u64, // seconds
    pub periods_to_keep: u64,
}

#[cw_serde]
pub struct ExchangeRate {
    pub height: u64,
    pub timestamp: Timestamp,
    pub exchange_rate: Decimal,
}

#[cw_serde]
pub struct Apy {
    pub start_exchange_rate: ExchangeRate,
    pub end_exchange_rate: ExchangeRate,
    pub apy: Decimal,
}

#[cw_serde]
pub enum CoreQueryMsg {
    ExchangeRate {},
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const LAST_ER_UPDATE: Item<Timestamp> = Item::new("last_er_update");
pub const NEXT_HOUR_COUNTER: Item<u64> = Item::new("next_hour_counter");
// instance name, hour index
pub const EXCHANGE_RATE: Map<u64, ExchangeRate> = Map::new("exchange_rate");
