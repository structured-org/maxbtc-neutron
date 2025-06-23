use crate::state::Config;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Coin;

#[cw_serde]
pub struct InstantiateMsg {
    // The address that can change the configuration.
    pub owner: String,
    // The address that can send the Push message.
    pub executor: String,
    // The denomination of the coin that needs to be sent to Ethereum.
    pub transfer_denom: String,
    // The eureka fee receiver address on Cosmos Hub.
    pub eureka_fee_receiver: String,
    // The recover address on Cosmos Hub.
    pub recover_address: String,
    // The source port on Neutron (usually "transfer").
    pub neutron_source_port: String,
    // The source port on Neutron (usually "channel-1").
    pub neutron_source_channel: String,
    // The Eureka source channel on Cosmos Hub (usually "08-wasm-1369", but can be changed
    // by Skip). Executor gets this value from Skip API and adds to the Push message, and we
    // return an error in case this value is different to what we expect — just in case.
    // Owner will need to reconfigure the contract with the new value to continue the operation.
    pub eureka_source_channel: String,
    // The contract on Cosmos Hub to which we send the original transfer. The comment to
    // `eureka_source_channel` applies here as well.
    pub to_chain_entry_contract_address: String,
    // The contract which is called by the entry contract on Cosmos Hub to actually process the
    // transfer. The comment to `eureka_source_channel` applies here as well.
    pub to_chain_callback_contract_address: String,
    // We refuse to pay more than `max_fee` for a Eureka transfer.
    pub max_fee: Coin,
    // Should always be false, but we keep this a parameter just in case something changes
    // in how Eureka works.
    pub exact_out: bool,
    // Fee for Neutron tIBC transfers, will be deprecated soon by the project.
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
        eureka_source_channel: String,
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
