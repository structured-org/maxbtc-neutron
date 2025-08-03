use cosmwasm_std::{Addr, Decimal, Timestamp};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The address authorized to claim fees and update the config.
    pub owner: Addr,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// The timestamp of the last successful fee collection.
    pub last_collection_timestamp: Timestamp,
    /// The exchange rate recorded after the last fee collection.
    pub last_exchange_rate: Decimal,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
