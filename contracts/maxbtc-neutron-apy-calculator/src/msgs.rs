use cosmwasm_schema::{cw_serde, QueryResponses};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

#[allow(unused_imports)]
use crate::state::{Apy, Config};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub core_contract: String,
    pub period_timeout: u64, // seconds
    pub periods_to_keep: u64,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateExchangeRates {},
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Apy)]
    GetApy { time_span_hours: u64 },
    #[returns(Config)]
    GetConfig {},
}

#[cw_serde]
pub enum MigrationMsg {
    Migrate {},
}
