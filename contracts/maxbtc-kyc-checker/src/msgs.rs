use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Deps, StdResult};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

#[allow(unused_imports)]
use crate::state::{Config, ConfigOptional};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub zkme_verify_upgradeable_contract: String,
    pub cooperator_address: String,
}

impl InstantiateMsg {
    pub fn into_config(self, deps: Deps) -> StdResult<Config> {
        Ok(Config {
            zkme_verify_upgradeable_contract: deps
                .api
                .addr_validate(&self.zkme_verify_upgradeable_contract)?,
            cooperator_address: self.cooperator_address,
        })
    }
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig { new_config: ConfigOptional },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(bool)]
    HasApproved { user: String },
    #[returns(Config)]
    GetConfig {},
}
