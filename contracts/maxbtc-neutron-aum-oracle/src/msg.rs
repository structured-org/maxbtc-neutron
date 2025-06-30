use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub aum: Option<Uint128>,
}

#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig { aum: Uint128 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Uint128)]
    GetAUM {},
}
