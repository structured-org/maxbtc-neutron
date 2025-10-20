use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, SignedDecimal256, Uint128};
use cw_storage_plus::{Item, Map};
use maxbtc_helpers::fsm::{Fsm, Transition};

#[cw_serde]
pub struct Config {
    /// When `true`, user-initiated actions are rejected; can be set
    /// automatically on emergencies or manually by the owner.
    pub paused: bool,
    /// Operator address
    pub operator: Addr,
    /// Admin contract with high privileges
    pub factory_contract: Addr,
    /// Contract that owns and creates token factory tokens.
    pub token_contract: Addr,
    /// Contract that forwards freshly-received deposits to the custody chain.
    pub deposit_forwarder_contract: Addr,
    /// Denom for user deposits (e.g. IBC-transferred BTC)
    pub deposit_denom: String,
    /// Number of decimals carried by the `deposit_denom` asset
    pub deposit_decimals: u32,
    /// One-off cost (Decimal) charged when a user deposits to mint maxBTC
    pub deposit_cost: Decimal,
    /// Optional upper limit on total AUM; deposits are rejected once the cap
    /// (if present) is exceeded
    pub deposits_cap: Option<Uint128>,
    /// Contract that holds amount of BTC received from CEFFU
    pub waitosaur_holder_contract: Addr,
    /// Contract address of the allow-list contract that manages
    /// the list of addresses allowed or passed KYC to mint maxBTC
    pub allowlist_contract: Addr,
    /// This contract provides the exchange rate for maxBTC
    pub exchange_rate_provider_contract: Addr,
    /// This contract is allowed to mint maxBTC to take a fee on the
    /// accrued protocol APR
    pub fee_collector_contract: Addr,
    /// Address of the waitosaur contract
    pub waitosaur_contract: Addr,
}

/// A single global config item
pub const CONFIG: Item<Config> = Item::new("config");

/// Total amount of BTC deposited by the contract
pub const TOTAL_DEPOSITED: Item<Uint128> = Item::new("total_deposited");

/// Deposit balance holder
pub const CURRENT_DEPOSIT_BALANCE: Item<Uint128> = Item::new("current_deposit_balance");

#[cw_serde]
pub enum ContractState {
    Idle,
    DepositNeutron,
    DepositPending,
    DepositJLP,
    WithdrawJLP,
    WithdrawPending,
    WithdrawNeutron,
}

#[cw_serde]
pub enum WaitosaurState {
    Locked {
        amount: SignedDecimal256,
        at_timestamp: u64,
    },
    Unlocked {},
}

const TRANSITIONS: &[Transition<ContractState>] = &[
    Transition {
        from: ContractState::Idle,
        to: ContractState::DepositNeutron,
    },
    Transition {
        from: ContractState::Idle,
        to: ContractState::WithdrawJLP,
    },
    Transition {
        from: ContractState::DepositNeutron,
        to: ContractState::DepositPending,
    },
    Transition {
        from: ContractState::DepositPending,
        to: ContractState::DepositJLP,
    },
    Transition {
        from: ContractState::DepositJLP,
        to: ContractState::Idle,
    },
    Transition {
        from: ContractState::WithdrawJLP,
        to: ContractState::WithdrawPending,
    },
    Transition {
        from: ContractState::WithdrawPending,
        to: ContractState::WithdrawNeutron,
    },
    Transition {
        from: ContractState::WithdrawNeutron,
        to: ContractState::Idle,
    },
];

pub const FSM: Fsm<ContractState> = Fsm::new("machine_state", TRANSITIONS);

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

/// The current active batch
pub const ACTIVE_BATCH: Item<Batch> = Item::new("active_batch");

/// The current withdrawing batch
pub const WITHDRAWING_BATCH: Item<Option<Batch>> = Item::new("withdrawing_batch");

/// Mapping from batch_id to a Batch (which will be in FINALIZED state)
pub const FINALIZED_BATCHES: Map<u64, Batch> = Map::new("finalized_batches");

/// A simple incrementing batch counter
pub const BATCH_ID_COUNTER: Item<u64> = Item::new("batch_id_counter");
