use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

use crate::state::waitosaur_holder::Config;

#[cw_serde]
pub struct InstantiateMsg {
    pub config: Config,
    pub owner: String,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig { new_config: UpdateConfig },
    Lock { amount: Uint128 },
    Unlock {},
}

#[cw_serde]
pub struct UpdateConfig {
    pub locker: Option<String>,
    pub unlocker: Option<String>,
    pub withdraw_manager_contract: Option<String>,
    pub asset: Option<String>,
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    /// Returns contract's configuration.
    GetConfig {},

    #[returns(crate::state::waitosaur_holder::State)]
    GetState {},
}

#[cw_serde]
pub struct MigrateMsg {}
