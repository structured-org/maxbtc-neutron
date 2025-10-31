use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::Item;

#[cw_serde]
pub struct CodeIds {
    pub token_code_id: u64,
    pub core_code_id: u64,
    pub deposit_forwarder_contract_code_id: u64,
    pub deposit_forwarder_library_contract_code_id: u64,
    pub exchange_rate_provider_contract_code_id: u64,
    pub allowlist_contract_code_id: u64,
    pub fee_collector_contract_code_id: u64,
    pub waitosaur_contract_code_id: u64,
}

#[cw_serde]
pub struct State {
    pub allowlist_contract: Addr,
    pub exchange_rate_provider_contract: Addr,
    pub fee_collector_contract: Addr,
    pub token_contract: Addr,
    pub deposit_forwarder_contract: Addr,
    pub deposit_forwarder_library_contract: Addr,
    pub core_contract: Addr,
    pub waitosaur_contract: Addr,
}

#[cw_serde]
pub struct WaitosaurConfig {
    pub locker: Addr,
    pub unlocker: Addr,
    pub contract: Addr,
    pub asset: String,
}

pub const STATE: Item<State> = Item::new("state");
