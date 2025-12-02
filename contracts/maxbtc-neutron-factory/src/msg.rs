use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{CosmosMsg, Decimal, Uint128, Uint64};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

use crate::state::{CodeIds, WaitosaurObserverConfig};

/// InstantiateMsg configures the contract on initialization.
#[cw_serde]
pub struct InstantiateMsg {
    pub code_ids: CodeIds,
    pub salt: String,
    pub owner: String,
    pub operator: String,
    pub ceffu_backend: String,
    /// Denom for user deposits (e.g. IBC-transferred BTC)
    pub deposit_denom: String,
    /// Number of decimals carried by the `deposit_denom` asset
    pub deposit_decimals: u32,
    /// One-off cost (Decimal) charged when a user deposits to mint maxBTC
    pub deposit_cost: Decimal,
    /// One-off cost (Decimal) charged when a user withdraws from maxBTC
    pub withdrawal_cost: Decimal,
    /// Upper limit on total AUM; deposits are rejected once the cap
    /// (if present) is exceeded
    pub deposits_cap: Option<Uint128>,
    /// Token factory sub-denom
    pub maxbtc_denom: String,
    /// Instantiation parameters for the fee collector.
    pub fee_collector_params: FeeMinterParams,
    /// Address of the binance AUM contract
    pub binance_aum_contract: String,
    /// Address of the waitosaur observer unlocker
    pub waitosaur_observer_unlocker: String,
    /// Address of the deposit forwarder contract
    pub deposit_forwarder_contract: String,
    /// Exchange rate timeout in seconds
    pub exchange_rate_stale_period: Uint64,
}

#[cw_serde]
pub struct WaitosaurObserverInstantiateMsg {
    pub config: WaitosaurObserverConfig,
    pub owner: String,
}

/// New struct to hold parameters for instantiating the fee collector contract.
#[cw_serde]
pub struct FeeMinterParams {
    /// The percentage of APY to be taken as a fee.
    pub fee_apy_reduction_percentage: Decimal,
    /// The duration in hours for each fee collection period.
    pub collection_period_seconds: u64,
}

/// Message for updating configuration parameters (owner-only).
#[cw_serde]
pub struct UpdateConfigMsg {
    pub paused: Option<bool>,
    pub deposit_forwarder_contract: Option<String>,
    pub deposit_flush_period: Option<u64>,
    pub deposit_cost: Option<Decimal>,
    pub exchange_rate_provider_contract: Option<String>,
    pub deposits_cap: Option<Option<Uint128>>,
    pub allowlist_contract: Option<String>,
    pub fee_collector_contract: Option<String>,
}

/// ExecuteMsg enumerates all possible actions in this contract.
#[cw_ownable_execute]
#[cw_serde]
#[allow(clippy::large_enum_variant)]
pub enum ExecuteMsg {
    AdminExecute { msgs: Vec<CosmosMsg> },
}

/// QueryMsg for reading contract states.
#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(crate::state::State)]
    State {},
}

#[cw_serde]
pub struct MigrateMsg {}
