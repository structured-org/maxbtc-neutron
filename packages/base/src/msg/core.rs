use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Decimal, Int256, SignedDecimal256, Uint128, Uint64};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

/// InstantiateMsg configures the contract on initialization.
#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    /// Operator address
    pub operator: String,
    /// Contract that owns and creates token factory tokens.
    pub token_contract: String,
    /// Admin contract with high privileges
    pub factory_contract: String,
    /// Contract that forwards freshly-received deposits to the custody chain.
    pub deposit_forwarder_contract: String,
    /// Denom for user deposits (e.g. IBC-transferred BTC)
    pub deposit_denom: String,
    /// Number of decimals carried by the `deposit_denom` asset
    pub deposit_decimals: u32,
    /// One-off cost (Decimal) charged when a user deposits to mint maxBTC
    pub deposit_cost: Decimal,
    /// Upper limit on total AUM; deposits are rejected once the cap
    /// (if present) is exceeded
    pub deposits_cap: Option<Uint128>,
    /// This contract provides the exchange rate for maxBTC
    pub exchange_rate_provider_contract: String,
    /// Exchange rate timeout in seconds
    pub exchange_rate_stale_period: Uint64,
    /// Contract address of the allow-list contract that manages
    /// the list of addresses allowed or passed KYC to mint maxBTC
    pub allowlist_contract: String,
    /// This contract is allowed to mint maxBTC to take a fee on the
    /// accrued protocol APR
    pub fee_collector_contract: String,
    /// Address of the waitosaur contract
    pub waitosaur_observer_contract: String,
    /// Address of the waitosaur holder contract
    pub waitosaur_holder_contract: String,
    /// Address of the withdrawal manager contract
    pub withdrawal_manager_contract: String,
}

/// Message for updating configuration parameters (owner-only).
#[cw_serde]
pub struct UpdateConfigMsg {
    pub paused: Option<bool>,
    pub operator: Option<String>,
    pub deposit_forwarder_contract: Option<String>,
    pub deposit_cost: Option<Decimal>,
    pub exchange_rate_provider_contract: Option<String>,
    pub exchange_rate_stale_period: Option<Uint64>,
    pub deposits_cap: Option<Option<Uint128>>,
    pub allowlist_contract: Option<String>,
    pub fee_collector_contract: Option<String>,
    pub waitosaur_observer_contract: Option<String>,
    pub waitsaur_holder_contract: Option<String>,
    pub withdrawal_manager_contract: Option<String>,
}

/// ExecuteMsg enumerates all possible actions in this contract.
#[cw_ownable_execute]
#[cw_serde]
#[allow(clippy::large_enum_variant)]
pub enum ExecuteMsg {
    Tick {},
    /// User deposit flow
    Deposit {
        recipient: String,
        min_receive_amount: Option<Uint128>,
    },
    /// User withdraw flow
    Withdraw {},
    /// Owner-only message to update protocol configuration in-place
    UpdateConfig(UpdateConfigMsg),
    /// Mints the requested amount of fees to the fee collector address. Can only be
    /// executed by the fee collector.
    MintFee {
        amount: Coin,
    },
    /// Mints the requested amount of maxBtc to the recevier address. Can only be
    /// executed by the owner.
    MintByOwner {
        amount: Uint128,
        recipient: String,
    },
}

/// QueryMsg for reading contract states.
#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(crate::state::core::ContractState)]
    ContractState {},
    #[returns(crate::state::core::Batch)]
    ActiveBatch {},
    #[returns(crate::state::core::Batch)]
    WithdrawingBatch {},
    #[returns(Vec<crate::state::core::Batch>)]
    FinalizedBatches {
        limit: Option<u32>,
        start_after: Option<Uint64>,
    },
    #[returns(crate::state::core::Batch)]
    FinalizedBatch { batch_id: u64 },
    /// Returns the Config state
    #[returns(crate::state::core::Config)]
    Config {},
    #[returns(Decimal)]
    ExchangeRate {},
    /// Simulates a deposit and returns the amount of maxBTC that would be minted.
    #[returns(SimulateDepositResponse)]
    SimulateDeposit { amount: Uint128 },
}

#[cw_serde]
pub struct SimulateDepositResponse {
    pub minted_amount: Uint128,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum AllowlistQueryMsg {
    #[returns(bool)]
    IsAddressAllowed { address: String },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum ExchangeRateProviderQueryMsg {
    #[returns(GetTwaerResponse)]
    GetTwaer {},
    #[returns(GetAumResponse)]
    GetAum {},
}

#[cw_serde]
pub struct GetAumResponse {
    pub aum_in_wbtc: Int256,
    pub decimals: u32,
}

#[cw_serde]
pub struct GetTwaerResponse {
    pub twaer: Decimal,
    pub published_at: u64,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum WaitosaurObserverQueryMsg {
    #[returns(crate::state::core::WaitosaurObserverState)]
    GetState {},
}

#[cw_serde]
pub enum WaitosaurObserverExecuteMsg {
    Lock { amount: SignedDecimal256 },
}

#[cw_serde]
pub struct MigrateMsg {}
