use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Decimal, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub core_contract_address: String,
    pub price_multiplier_basis_points: u128,
    pub btc_denom: String,
    pub maxbtc_denom: String,
    pub authority: String,
    pub treasury: String,
}

#[cw_serde]
pub struct UpdateConfig {
    pub core_contract_address: Option<String>,
    pub btc_denom: Option<String>,
    pub maxbtc_denom: Option<String>,
    pub price_multiplier_basis_points: Option<u128>,
    pub authority: Option<String>,
    pub treasury: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    // Only core contract can call ClawBack.
    // returns the specified amount of BTC to the core contract via the following sequence:
    // [withdraw active order(s)] -> [send BTC to core contract] -> [re-place limit order with new amount]
    ClawBack { amount: Coin },
    // Only authority address can call this.
    // updates the config
    UpdateConfig { config: UpdateConfig },
    // runs the sequence of actions:
    // [withdraw active order(s)] -> [burn maxBTC] -> [send maxBTC to treasury] -> [place new limit order]
    RunSequence {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Uint128)]
    GetBTCBalance {},
    #[returns(Uint128)]
    GetMaxBTCBalance {},
}
