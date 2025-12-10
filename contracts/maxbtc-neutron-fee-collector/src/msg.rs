use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Decimal};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub core_contract: String,
    pub fee_apy_reduction_percentage: Decimal,
    pub collection_period_seconds: u64,
    pub fee_denom: String,
    pub maxbtc_decimals: u32,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    /// Triggers the fee collection process.
    CollectFee {},
    /// Called by the owner to withdraw collected fees.
    Claim { amount: Coin, recipient: String },
    /// Called by the owner to update the contract configuration.
    UpdateConfig {
        core_contract: Option<String>,
        fee_apy_reduction_percentage: Option<Decimal>,
        collection_period_seconds: Option<u64>,
    },
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the current contract configuration.
    #[returns(crate::state::Config)]
    Config {},
    /// Returns the current contract state.
    #[returns(crate::state::State)]
    State {},
}

/// Query messages for the maxbtc-neutron-core contract.
#[cw_serde]
pub enum CoreQueryMsg {
    ExchangeRate {},
}

/// Execute messages for the maxbtc-neutron-core contract.
#[cw_serde]
pub enum CoreExecuteMsg {
    MintFee { amount: Coin },
}

#[cw_serde]
pub struct MigrateMsg {}
