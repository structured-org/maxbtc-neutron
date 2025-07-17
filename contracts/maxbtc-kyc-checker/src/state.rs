use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::Item;

#[cw_serde]
pub struct ConfigOptional {
    pub zkme_verify_upgradeable_contract: Option<String>,
    pub cooperator_address: Option<String>,
}

#[cw_serde]
pub struct Config {
    pub zkme_verify_upgradeable_contract: Addr,
    pub cooperator_address: String,
}

#[cw_serde]
pub enum ZkmeVerifyQueryMsg {
    HasApproved { cooperator: String, user: String },
}

pub const CONFIG: Item<Config> = Item::new("config");
