use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Coin;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    Claim { amount: Coin },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}
