use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, SignedDecimal256, Uint128};
use cw_storage_plus::Item;
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
    /// Minimum number of seconds between two deposit-flush operations
    pub deposit_flush_period: u64,
    /// One-off cost (Decimal) charged when a user deposits to mint maxBTC
    pub deposit_cost: Decimal,
    /// Optional upper limit on total AUM; deposits are rejected once the cap
    /// (if present) is exceeded
    pub deposits_cap: Option<Uint128>,
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

/// Tracks the last time a deposit flush was done
pub const LAST_DEPOSIT_FLUSH_TIME: Item<u64> = Item::new("last_deposit_flush_time");

/// Total amount of BTC deposited by the contract
pub const TOTAL_DEPOSITED: Item<Uint128> = Item::new("total_deposited");

#[cw_serde]
pub enum ContractState {
    Idle,
    DepositNeutron,
    DepositPending,
    DepositJLP,
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
];

pub const FSM: Fsm<ContractState> = Fsm::new("machine_state", TRANSITIONS);
