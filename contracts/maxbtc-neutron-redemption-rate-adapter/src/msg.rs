#[allow(unused_imports)]
use crate::state::Config;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Decimal};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig { new_config: UpdateConfig },
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    Config {},
    #[returns(RedemptionRateResponse)]
    RedemptionRate {
        denom: String,
        params: Option<Binary>,
    },
}

#[cw_serde]
pub struct RedemptionRateResponse {
    pub redemption_rate: Decimal,
    pub update_time: u64,
}

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub fee_bps: u16,
    pub twaer_provider_contract: String,
    pub denom: String,
}

#[cw_serde]
pub struct UpdateConfig {
    pub denom: Option<String>,
    pub fee_bps: Option<u16>,
    pub twaer_provider_contract: Option<String>,
}
