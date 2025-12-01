use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Timestamp};
use cw_storage_plus::Item;

#[cw_serde]
pub struct Config {
    /// The address of the maxbtc-neutron-core contract.
    pub core_contract: Addr,
    /// The percentage of the APY gain to be collected as a fee (e.g., 0.1 for 10%).
    pub fee_apy_reduction_percentage: Decimal,
    /// The minimum period in seconds between fee collections.
    pub collection_period_seconds: u64,
    /// The denom of the token to be minted and claimed.
    pub fee_denom: String,
    /// The number of decimals for the maxBTC token.
    pub maxbtc_decimals: u32,
}

#[cw_serde]
pub struct State {
    /// The timestamp of the last successful fee collection.
    pub last_collection_timestamp: Timestamp,
    /// The exchange rate recorded after the last fee collection.
    pub last_exchange_rate: Decimal,
}

pub const CONFIG: Item<Config> = Item::new("config_v2");
pub const STATE: Item<State> = Item::new("state");
