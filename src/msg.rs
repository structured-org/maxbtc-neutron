use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Decimal};

/// InstantiateMsg configures the contract on initialization.
#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub aum_contract: String,
    pub liquidation_contract: String,
    pub collector_contract: String,
    pub treasury_address: String,
    /// Denom for user deposits (e.g. the IBC-transferred BTC)
    pub deposit_denom: String,
    /// Number of decimals in the deposits coin
    pub deposit_decimals: u32,
    /// The tokenfactory denom representing the maxBTC token
    pub maxbtc_denom: String,
    /// Duration in seconds after which deposit flush can be triggered
    pub deposit_flush_period: u64,
    /// Duration in seconds after which an active batch transitions to WITHDRAWING
    pub batch_active_duration: u64,
    /// Duration in seconds after which a withdrawing batch transitions to FINALIZED
    pub batch_withdrawing_duration: u64,
    /// If the collected amount in the collector account is less than this % of requested,
    /// the protocol goes into paused state.
    pub accepted_withdrawable_percentage: Decimal,
    /// The share (in decimal) of AUM we want to hold in the liquidation contract
    pub liquidation_buffer_share: Decimal,
    /// The deposit fee (applied at the time of deposit)
    pub deposit_fee: Decimal,
}

/// ExecuteMsg enumerates all possible actions in this contract.
#[cw_serde]
pub enum ExecuteMsg {
    /// User deposit flow
    Deposit { recipient: String },
    /// Permissionless handler to flush deposits after `deposit_flush_period`
    FlushDeposits {},
    /// User requests a withdrawal of a certain amount of maxBTC
    Withdraw {
        amount: Coin, // expected to be the maxBTC denom
    },
    /// Permissionless handler to process the ACTIVE batch after `batch_active_duration`
    ProcessActiveBatch {},
    /// Permissionless handler to finalize the WITHDRAWING batch after `batch_withdrawing_duration`
    FinalizeWithdrawingBatch {},
    /// User claims their BTC from a finalized batch
    Claim {
        /// The user wants to receive BTC at `recipient` address on Neutron
        /// (which the user can IBC-transfer out later).
        recipient: String,
    },
}

/// QueryMsg for reading contract states.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the Config state
    #[returns(ConfigResponse)]
    Config {},
    /// Returns info about the current ACTIVE batch
    #[returns(Option<BatchResponse>)]
    ActiveBatch {},
    /// Returns info about the current WITHDRAWING batch
    #[returns(Option<BatchResponse>)]
    WithdrawingBatch {},
    /// Returns info for a finalized batch by id
    #[returns(Option<BatchResponse>)]
    FinalizedBatch { batch_id: u64 },
}

/// Response for querying config
#[cw_serde]
pub struct ConfigResponse {
    pub owner: String,
    pub aum_contract: String,
    pub liquidation_contract: String,
    pub treasury_address: String,
    pub deposit_denom: String,
    pub maxbtc_denom: String,
    pub deposit_flush_period: u64,
    pub batch_active_duration: u64,
    pub batch_withdrawing_duration: u64,
    pub accepted_withdrawable_percentage: Decimal,
    pub liquidation_buffer_share: Decimal,
    pub deposit_fee: Decimal,
}

/// Batch status
#[cw_serde]
pub enum BatchStatus {
    Active,
    Withdrawing,
    Finalized,
}

/// Response for batch query
#[cw_serde]
pub struct BatchResponse {
    pub batch_id: u64,
    pub status: BatchStatus,
    pub btc_requested: String,
    pub collected_amount: String,
    pub collector_historical_balance: String,
}

#[cw_serde]
pub enum AUMQueryMsg {
    GetAUM {},
}

#[cw_serde]
pub enum LiquidationExecuteMsg {
    ClawBack { amount: Coin },
}
