use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub owned_maxbtc: Option<Uint128>,
    pub owned_btc: Option<Uint128>,
}

#[cw_serde]
pub enum ExecuteMsg {
    ClawBack {
        amount: Coin,
    },
    UpdateConfig {
        owned_maxbtc: Uint128,
        owned_btc: Uint128,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Uint128)]
    GetBTCBalance {},
    #[returns(Uint128)]
    GetMaxBTCBalance {},
}
