use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Coin};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

/// InstantiateMsg configures the contract on initialization.
#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    /// Factory contract that owns all admin privileges
    pub factory_contract: String,
    /// The token-factory sub-denom used for the maxBTC token
    pub subdenom: String,
}

/// Message for updating configuration parameters (owner-only).
#[cw_serde]
pub struct UpdateConfigMsg {
    pub factory_contract: Option<String>,
}

/// ExecuteMsg enumerates all possible actions in this contract.
#[cw_ownable_execute]
#[cw_serde]
#[allow(clippy::large_enum_variant)]
pub enum ExecuteMsg {
    /// User deposit flow
    Mint { amount: Coin, recipient: String },
    /// User withdraw flow, burns sent coins
    Burn {},
    /// Set tokenfactory denom metadata
    SetTokenMetadata { token_metadata: DenomMetadata },
    /// Creates redemption token
    CreateRedemptionToken { redemption_subdenom: String },
    /// Updates contract configuration
    UpdateConfig { factory_contract: Option<String> },
}

#[cw_serde]
pub struct DenomMetadata {
    /// Number of decimals
    pub exponent: u32,
    /// Lowercase moniker to be displayed in clients, example: "atom"
    pub display: String,
    /// Descriptive token name, example: "Cosmos Hub Atom"
    pub name: String,
    /// Even longer description, example: "The native staking token of the Cosmos Hub"
    pub description: String,
    /// Symbol to be displayed on exchanges, example: "ATOM"
    pub symbol: String,
    /// URI to a document that contains additional information
    pub uri: Option<String>,
    /// SHA256 hash of a document pointed by URI
    pub uri_hash: Option<String>,
}

/// QueryMsg for reading contract states.
#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the Config state
    #[returns(crate::state::token::Config)]
    Config {},
    /// Returns the token factory denom
    #[returns(String)]
    GetDenom { subdenom: Option<String> },
}

#[cw_serde]
pub struct MigrateMsg {
    pub waitosaur_observer_code_id: u64,
    pub waitosaur_holder_code_id: u64,
    pub core_code_id: u64,
    pub withdrawal_manager_code_id: u64,
    pub factory_contract: Addr,
    pub operator: Addr,
    pub waitosaur_observer_unlocker: String,
    pub binance_aum_contract: String,
    pub ceffu_backend: Addr,
    pub salt: String,
}
