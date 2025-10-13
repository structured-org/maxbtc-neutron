use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::Item;

#[cw_serde]
pub struct Config {
    /// When `true`, user-initiated actions are rejected; can be set
    /// automatically on emergencies or manually by the owner.
    pub paused: bool,
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
}

impl Config {
    pub fn get_redemption_denom(&self, _contract_addr: String, _batch_id: u64) -> String {
        "".to_string()
    }
}

/// A single global config item
pub const CONFIG: Item<Config> = Item::new("config");

/// Tracks the last time a deposit flush was done
pub const LAST_DEPOSIT_FLUSH_TIME: Item<u64> = Item::new("last_deposit_flush_time");

/// Total amount of BTC deposited by the contract
pub const TOTAL_DEPOSITED: Item<Uint128> = Item::new("total_deposited");
