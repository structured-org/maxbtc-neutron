use cosmwasm_std::{Addr, Coin};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub transfer_denom: String,
    pub to_chain_receiver: String,
    pub to_chain_recover_address: String,
    pub to_chain_source_channel: String,
    pub to_chain_entry_contract_address: String,
    pub to_chain_callback_contract_address: String,
    pub max_fee: Coin,
    pub oracle_address: String,
    pub relay_fee: Coin,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Transfer {
        amount: Coin,
        eureka_fee: EurekaFee,
        // These are fetched from an oracle for security
        oracle_entry_address: String,
        oracle_callback_address: String,
        // Expected format: 1750089037000000000 (unix nano)
        timeout_timestamp_ibc: u64,
        // Expected format: 1750089037000000000 (unix nano)
        timeout_timestamp_eureka: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

// Structs for building the complex memo
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Memo {
    pub dest_callback: DestCallback,
    pub wasm: Wasm,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DestCallback {
    pub address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Wasm {
    pub contract: String,
    pub msg: WasmMsg,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WasmMsg {
    pub action: Action,
    pub exact_out: bool,
    pub timeout_timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Action {
    pub action: IbcTransferAction,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IbcTransferAction {
    IbcTransfer(IbcTransfer),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct IbcTransfer {
    pub ibc_info: IbcInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct IbcInfo {
    pub encoding: String,
    pub eureka_fee: EurekaFee,
    pub memo: String,
    pub receiver: String,
    pub recover_address: String,
    pub source_channel: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct EurekaFee {
    pub coin: Coin,
    pub receiver: String,
    pub timeout_timestamp: u64,
}
