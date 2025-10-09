use crate::state::withdrawal_manager::Pause;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

#[cw_serde]
pub struct InstantiateMsg {
    pub factory_contract: String,
    pub core_contract: String,
    pub token_contract: String,
    pub deposit_denom: String,
    pub owner: String,
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(crate::state::withdrawal_manager::Config)]
    Config {},
    #[returns(Pause)]
    Pause {},
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig {
        factory_contract: Option<String>,
        core_contract: Option<String>,
        token_contract: Option<String>,
        deposit_denom: Option<String>,
    },
    /// Claim withdrawal using redemption token
    Claim {
        recipient: String,
    },
    SetPause {
        pause: Pause,
    },
}

#[cw_serde]
pub struct MigrateMsg {}
