use crate::fsm::{Fsm, Transition};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    /// When `true`, user-initiated actions are rejected; can be set
    /// automatically on emergencies or manually by the owner.
    pub paused: bool,
    /// Address with full administrative rights over the contract.
    pub owner: Addr,
    /// Address of the oracle contract that reports total AUM.
    pub aum_oracle_contract: Addr,
    /// Address of the contract that manages the liquidation buffer.
    pub liquidation_buffer_contract: Addr,
    /// Contract that forwards freshly-received deposits to the custody chain.
    pub deposit_pump_contract: Addr,
    /// Collector account that receives BTC shipped back from custody
    /// during the withdrawal process.
    pub collector_contract: Addr,
    /// Treasury account that receives protocol fees and surplus funds.
    pub treasury_address: Addr,
    /// Denom for user deposits (e.g. IBC-transferred BTC)
    pub deposit_denom: String,
    /// Number of decimals carried by the `deposit_denom` asset
    pub deposit_decimals: u32,
    /// Token-factory sub-denom used for maxBTC
    pub maxbtc_denom: String,
    /// Minimum number of seconds between two deposit-flush operations
    pub deposit_flush_period: u64,
    /// Seconds an ACTIVE batch remains open before promotion to WITHDRAWING
    pub batch_active_duration: u64,
    /// Seconds a WITHDRAWING batch may remain open before finalization
    pub batch_withdrawing_duration: u64,
    /// Minimum percentage (Decimal) of `btc_requested` that must be
    /// collected for a batch to finalize successfully
    pub collected_tolerance: Decimal,
    /// Fraction of total AUM (Decimal) that the protocol must keep in the
    /// liquidation buffer contract as an instant-liquidity buffer
    pub liquidation_buffer_share: Decimal,
    /// One-off cost (Decimal) charged when a user deposits to mint maxBTC
    pub deposit_cost: Decimal,
    /// Maximum tolerated relative difference (Decimal) between the deposit
    /// buffer sent for flushing and the amount observed on the custody chain
    pub deposit_buffer_tolerance: Decimal,
    /// Lifetime, in seconds, of the cached ER/AUM snapshot while a multi-step
    /// operation is in flight
    pub cached_er_ttl: u64,
    /// Optional upper limit on total AUM; deposits are rejected once the cap
    /// (if present) is exceeded
    pub deposits_cap: Option<Uint128>,
    /// Optional allow-list of addresses that may mint maxBTC; `None` or an
    /// empty vector means deposits are open to everyone
    pub deposits_allowlist: Option<Vec<Addr>>,
    /// This contract is allowed to mint maxBTC to take a fee on the
    /// accrued protocol APR
    pub fee_collector_contract: Addr,
}

impl Config {
    pub fn get_maxbtc_denom(&self, contract_addr: String) -> String {
        format!("factory/{}/{}", contract_addr, self.maxbtc_denom)
    }

    pub fn get_redemption_denom(&self, contract_addr: String, batch_id: u64) -> String {
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
pub const CACHED_ER: Item<Option<CachedER>> = Item::new("cached_er");
