use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateAllowList { allow_list: Vec<String> },
    UpdateZkMeSettings { settings: Option<ZkMeSettings> },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct ZkMeSettings {
    pub contract: Addr,
    pub cooperator: Addr,
}

#[cw_serde]
pub enum ZkMeQueryMsg {
    HasApproved { user: Addr, cooperator: Addr },
}

#[cw_serde]
pub struct ZkMeHasApprovedResponse {
    pub cooperator: Addr,
    pub user: Addr,
    pub has_approved: bool,
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Vec<String>)]
    AllowList {},
    #[returns(bool)]
    IsAddressAllowed { address: String },
}
