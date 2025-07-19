use cosmwasm_schema::{cw_serde, QueryResponses};
use cw_ownable::cw_ownable_execute;

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    SetKYC { address: String, kyc: bool },
    UpdateAllowList { allow_list: Vec<String> },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(String)]
    Owner {},
    #[returns(Vec<String>)]
    AllowList {},
    #[returns(bool)]
    KYCCheck { address: String },
    #[returns(bool)]
    IsAddressAllowed { address: String },
}
