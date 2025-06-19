use crate::state::Config;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Coin;

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub transfer_denom: String,
    pub receiver: String,
    pub recover_address: String,
    pub source_port: String,
    pub source_channel: String,
    pub to_chain_entry_contract_address: String,
    pub to_chain_callback_contract_address: String,
    pub max_fee: Coin,
    pub oracle_address: String,
    pub exact_out: bool,
    pub relay_fee: Coin,
}

#[cw_serde]
pub enum ExecuteMsg {
    Push {
        amount: Coin,
        eureka_fee: EurekaFee,
        // These are fetched from an oracle for security
        oracle_entry_address: String,
        oracle_callback_address: String,
        source_channel: String,
        // Expected format: 1750089037000000000 (unix nano)
        eureka_fee_timeout_nano: u64,
        // Expected format: 1750089037000000000 (unix nano)
        eureka_full_timeout_nano: u64,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    Config {},
}

// Structs for building the complex memo
#[cw_serde]
pub struct Memo {
    pub dest_callback: DestCallback,
    pub wasm: Wasm,
}

#[cw_serde]
pub struct DestCallback {
    pub address: String,
}

#[cw_serde]
pub struct Wasm {
    pub contract: String,
    pub msg: WasmMsg,
}

#[cw_serde]
pub struct WasmMsg {
    pub action: Action,
    pub exact_out: bool,
    pub timeout_timestamp: u64,
}

#[cw_serde]
pub struct Action {
    pub action: IbcTransferAction,
}

#[cw_serde]
pub enum IbcTransferAction {
    IbcTransfer(IbcTransfer),
}

#[cw_serde]
pub struct IbcTransfer {
    pub ibc_info: IbcInfo,
}

#[cw_serde]
pub struct IbcInfo {
    pub encoding: String,
    pub eureka_fee: EurekaFee,
    pub memo: String,
    pub receiver: String,
    pub recover_address: String,
    pub source_channel: String,
}

#[cw_serde]
pub struct EurekaFee {
    pub coin: Coin,
    pub receiver: String,
    pub timeout_timestamp: u64,
}
