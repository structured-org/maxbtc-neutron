use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Coin, Decimal, Uint128};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

/// InstantiateMsg configures the contract on initialization.
#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
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
    /// Minimum number of seconds that must elapse between two deposit-flush operations
    pub deposit_flush_period: u64,
    /// One-off cost (Decimal) charged when a user deposits to mint maxBTC
    pub deposit_cost: Decimal,
    /// Upper limit on total AUM; deposits are rejected once the cap
    /// (if present) is exceeded
    pub deposits_cap: Option<Uint128>,
    /// This contract provides the exchange rate for maxBTC
    pub exchange_rate_provider_contract: String,
    /// Contract address of the allow-list contract that manages
    /// the list of addresses allowed or passed KYC to mint maxBTC
    pub allowlist_contract: String,

    //----------------------------------------------------------------------------------------
    /// Instantiation parameters for the fee collector.
    // pub fee_collector_params: FeeMinterParams,
    //----------------------------------------------------------------------------------------

    /// This contract is allowed to mint maxBTC to take a fee on the
    /// accrued protocol APR
    pub fee_collector_contract: String,
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
    /// User deposit flow
    Deposit {
        recipient: String,
        min_receive_amount: Option<Uint128>,
    },
    /// Permissionless handler to flush deposits after `deposit_flush_period`
    FlushDeposits {},
    /// Owner-only message to update protocol configuration in-place
    UpdateConfig(UpdateConfigMsg),
    /// Mints the requested amount of fees to the fee collector address. Can only be
    /// executed by the fee collector.
    MintFee { amount: Coin },
}

/// QueryMsg for reading contract states.
#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the Config state
    #[returns(ConfigResponse)]
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

/// Response for querying config
#[cw_serde]
pub struct ConfigResponse {
    pub deposit_denom: String,
    pub deposit_flush_period: u64,
    pub deposit_cost: Decimal,
    pub fee_collector_contract: String,
}

/// Describes the queries that can be sent to the liquidation buffer contract.
#[cw_serde]
pub enum LiquidationBufferContractQueryMsg {
    GetBTCBalance {},
    GetMaxBTCBalance {},
}

// (This is the InstantiateMsg for the fee collector contract, shown for context)
#[cw_serde]
pub struct FeeCollectorInstantiateMsg {
    pub owner: String,
    pub core_contract: String,
    pub fee_apy_reduction_percentage: Decimal,
    pub collection_period_seconds: u64,
    pub fee_denom: String,
    pub maxbtc_decimals: u32,
}

/// New struct to hold parameters for instantiating the fee collector contract.
#[cw_serde]
pub struct FeeMinterParams {
    /// The code ID of the fee collector contract wasm.
    pub code_id: u64,
    /// A unique salt for generating a predictable address with Instantiate2.
    pub salt: Binary,
    /// The percentage of APY to be taken as a fee.
    pub fee_apy_reduction_percentage: Decimal,
    /// The duration in seconds for each fee collection period.
    pub collection_period_seconds: u64,
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
}

#[cw_serde]
pub struct GetTwaerResponse {
    pub twaer: Decimal,
    pub published_at: u64,
}

#[cw_serde]
pub struct MigrateMsg {}
