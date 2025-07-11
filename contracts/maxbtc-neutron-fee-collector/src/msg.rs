use cosmwasm_std::{Coin, Decimal, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub core_contract: String,
    pub fee_apy_reduction_percentage: Decimal,
    pub collection_period_hours: u64,
    pub fee_denom: String,
    pub maxbtc_decimals: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Triggers the fee collection process.
    CollectFee {},
    /// Called by the owner to withdraw collected fees.
    Claim { amount: Coin, recipient: String },
    /// Called by the owner to update the contract configuration.
    UpdateConfig {
        owner: Option<String>,
        core_contract: Option<String>,
        fee_apy_reduction_percentage: Option<Decimal>,
        collection_period_hours: Option<u64>,
        maxbtc_decimals: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns the current contract configuration.
    Config {},
    /// Returns the current contract state.
    State {},
}

// Response for the Config query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub core_contract: String,
    pub fee_apy_reduction_percentage: Decimal,
    pub collection_period_seconds: u64,
    pub fee_denom: String,
    pub maxbtc_decimals: u32,
}

// Response for the State query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub last_collection_timestamp: Timestamp,
    pub last_exchange_rate: Decimal,
}

/// Query messages for the maxbtc-neutron-core contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoreQueryMsg {
    ExchangeRate {},
}

/// Response for the core contract's ExchangeRate query.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ExchangeRateResponse {
    pub rate: Decimal,
}

/// Execute messages for the maxbtc-neutron-core contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoreExecuteMsg {
    MintFee { amount: Coin },
}
