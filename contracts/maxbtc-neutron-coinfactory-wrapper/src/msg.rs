use cosmwasm_schema::{cw_serde, QueryResponses};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};
use maxbtc_base::msg::token::DenomMetadata;

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub allowed_denom: String,
    pub subdenom: String,
    pub token_metadata: DenomMetadata,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    Wrap {},
    Unwrap {},
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(crate::state::Config)]
    Config {},
    #[returns(String)]
    Denom {},
}
