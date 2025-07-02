use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, Uint128};
use cw_storage_plus::Item;

#[cw_serde]
pub struct Config {
    // MAXBTC-neutron core contract address
    pub core_contract_address: String,
    // denom for the Base BTC denom
    pub btc_denom: String,
    // denom MaxBTC
    pub maxbtc_denom: String,
    // price multiplier in basis points. (10000 = 100%)
    // this determines how much more expensive than market price (exchange rate)
    // the contract will sell BTC for
    pub price_multiplier_basis_points: u128,
    // authority address, has access to config update
    pub authority: String,
    // treasury address, receives MAXBTC from burning
    pub treasury: String,
}

/// A single global config item
pub const CONFIG: Item<Config> = Item::new("config");
