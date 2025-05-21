use crate::fsm::{Fsm, Transition};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    /// TODO
    pub paused: bool,
    /// TODO
    pub owner: Addr,
    /// TODO
    pub aum_contract: Addr,
    /// TODO
    pub liquidation_contract: Addr,
    /// TODO
    pub deposit_pump_contract: Addr,
    /// TODO
    pub collector_contract: Addr,
    /// TODO
    pub treasury_address: Addr,
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
    pub collected_tolerance: Decimal,
    /// The share (in decimal) of AUM we want to hold in the liquidation contract
    pub liquidation_buffer_share: Decimal,
    /// E.g. 0.003 for 0.3% deposit fee
    pub deposit_fee: Decimal,
    /// Are deposits/withdrawals paused? (Could be triggered by emergencies)
    /// TODO
    pub deposit_buffer_tolerance: Decimal,
    /// TODO
    pub cached_er_ttl: u64,
    /// TODO
    pub deposits_cap: Option<Uint128>,
    /// TODO
    pub deposits_allowlist: Option<Vec<Addr>>,
}

impl Config {
    pub fn get_maxbtc_denom(&self, contract_addr: String) -> String {
        format!(
            "factory/{}/{}",
            contract_addr,
            self.maxbtc_denom.to_string()
        )
    }

    pub fn get_redemption_denom(&self, contract_addr: String, batch_id: String) -> String {
        format!("factory/{}/redemption/batch/{}", contract_addr, batch_id)
    }
}

/// Each batch has a batch_id, which increments.
#[cw_serde]
pub struct Batch {
    pub batch_id: u64,
    /// If the batch is in WITHDRAWING or FINALIZED, how much BTC was requested?
    pub btc_requested: Uint128,
    /// The amount of maxBTC burned for this batch
    pub maxbtc_burned: Uint128,
    /// If in FINALIZED state, how much BTC was actually collected?
    pub collected_amount: Uint128,
    /// If in FINALIZED state, how much BTC was already paid to users?
    pub paid_amount: Uint128,
    /// Historical collector balance recorded at the time the batch transitions to WITHDRAWING
    pub collector_historical_balance: Uint128,
}

#[cw_serde]
pub struct CachedER {
    pub er: Decimal,
    pub timeout: u64,
    pub aum: Option<CachedAUM>,
}

#[cw_serde]
pub struct CachedAUM {
    pub oracle_aum: Uint128,
    pub deposit_buffer: Uint128,
}

/// Represents the contract state.
#[cw_serde]
pub enum ContractState {
    Idle,
    Flushing,
    Withdrawing,
}

/// Defines the valid state transitions of the contract.
const TRANSITIONS: &[Transition<ContractState>] = &[
    Transition {
        from: ContractState::Idle,
        to: ContractState::Flushing,
    },
    Transition {
        from: ContractState::Flushing,
        to: ContractState::Idle,
    },
    Transition {
        from: ContractState::Idle,
        to: ContractState::Withdrawing,
    },
    Transition {
        from: ContractState::Withdrawing,
        to: ContractState::Idle,
    },
];

/// The current status of the contract
pub const FSM: Fsm<ContractState> = Fsm::new("contract_state", TRANSITIONS);

/// The current active batch
pub const ACTIVE_BATCH: Item<Option<Batch>> = Item::new("active_batch");

/// The current withdrawing batch
pub const WITHDRAWING_BATCH: Item<Option<Batch>> = Item::new("withdrawing_batch");

/// Mapping from batch_id to a Batch (which will be in FINALIZED state)
pub const FINALIZED_BATCHES: Map<u64, Batch> = Map::new("finalized_batches");

/// A single global config item
pub const CONFIG: Item<Config> = Item::new("config");

/// A simple incrementing batch counter
pub const BATCH_ID_COUNTER: Item<u64> = Item::new("batch_id_counter");

/// Tracks the last time a deposit flush was done
pub const LAST_DEPOSIT_FLUSH_TIME: Item<u64> = Item::new("last_deposit_flush_time");

/// Tracks the time an active batch was initiated
pub const ACTIVE_BATCH_START_TIME: Item<u64> = Item::new("active_batch_start_time");

/// Cached assets under management value. Can be set when we trigger a deposits flush
/// and when we move a batch to the WITHDRAWING state.
pub const CACHED_ER: Item<Option<CachedER>> = Item::new("cached_aum");
