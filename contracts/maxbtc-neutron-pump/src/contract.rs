use crate::error::ContractError;
use crate::msg::{
    Action, DestCallback, EurekaFee, ExecuteMsg, IbcInfo, IbcTransfer, IbcTransferAction,
    InstantiateMsg, Memo, QueryMsg, Wasm, WasmMsg,
};
use crate::state::{Config, CONFIG};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use neutron_std::types::cosmos::base::v1beta1::Coin as StdCoin;
use neutron_std::types::neutron::feerefunder::Fee;
use neutron_std::types::neutron::transfer::MsgTransfer;

const CONTRACT_NAME: &str = "crates.io:maxbtc-neutron-pump";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let owner = deps.api.addr_validate(&msg.owner)?;
    let oracle_address = deps.api.addr_validate(&msg.oracle_address)?;

    let config = Config {
        owner,
        transfer_denom: msg.transfer_denom,
        to_chain_receiver: msg.to_chain_receiver,
        to_chain_recover_address: msg.to_chain_recover_address,
        to_chain_source_channel: msg.to_chain_source_channel,
        to_chain_entry_contract_address: msg.to_chain_entry_contract_address,
        to_chain_callback_contract_address: msg.to_chain_callback_contract_address,
        max_fee: msg.max_fee,
        oracle_address,
        relay_fee: msg.relay_fee,
    };
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", msg.owner))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer {
            amount,
            eureka_fee,
            oracle_entry_address,
            oracle_callback_address,
            timeout_timestamp_ibc,
            timeout_timestamp_eureka,
        } => execute_transfer(
            deps,
            env,
            info,
            amount,
            eureka_fee,
            oracle_entry_address,
            oracle_callback_address,
            timeout_timestamp_ibc,
            timeout_timestamp_eureka,
        ),
    }
}

fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: cosmwasm_std::Coin,
    eureka_fee: EurekaFee,
    oracle_entry_address: String,
    oracle_callback_address: String,
    timeout_timestamp_ibc: u64,
    timeout_timestamp_eureka: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Permission check: only the owner can execute this
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Fee check
    if eureka_fee.coin.amount > config.max_fee.amount
        || eureka_fee.coin.denom != config.max_fee.denom
    {
        return Err(ContractError::InvalidFee {});
    }

    // Oracle address validation
    if oracle_entry_address != config.to_chain_entry_contract_address {
        return Err(ContractError::OracleMismatch {});
    }
    if oracle_callback_address != config.to_chain_callback_contract_address {
        return Err(ContractError::OracleMismatch {});
    }

    // Check if the transfer denom matches the config
    if amount.denom != config.transfer_denom {
        return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
            "Invalid transfer denom",
        )));
    }

    // Construct the complex memo
    let memo = Memo {
        dest_callback: DestCallback {
            address: config.to_chain_entry_contract_address.clone(),
        },
        wasm: Wasm {
            contract: config.to_chain_callback_contract_address.clone(),
            msg: WasmMsg {
                action: Action {
                    action: IbcTransferAction::IbcTransfer(IbcTransfer {
                        ibc_info: IbcInfo {
                            encoding: "application/x-solidity-abi".to_string(),
                            eureka_fee: EurekaFee {
                                coin: eureka_fee.coin.clone(),
                                receiver: eureka_fee.receiver,
                                timeout_timestamp: timeout_timestamp_eureka,
                            },
                            memo: "".to_string(),
                            receiver: config.to_chain_receiver.clone(),
                            recover_address: config.to_chain_recover_address.clone(),
                            source_channel: config.to_chain_source_channel.clone(),
                        },
                    }),
                },
                exact_out: false,
                // Expects unix seconds instead of unix nano for some reason
                timeout_timestamp: timeout_timestamp_eureka / 1_000_000_000,
            },
        },
    };

    let memo_str = serde_json_wasm::to_string(&memo).unwrap();

    // Construct the IBC Transfer message
    let transfer_msg = MsgTransfer {
        source_port: "transfer".to_string(),
        source_channel: config.to_chain_source_channel,
        sender: env.contract.address.to_string(),
        receiver: config.to_chain_entry_contract_address, // The initial receiver on the dest chain
        token: Some(StdCoin {
            denom: amount.denom,
            amount: amount.amount.to_string(),
        }),
        timeout_height: None,
        timeout_timestamp: timeout_timestamp_ibc,
        memo: memo_str,
        // Will be soon deprecated
        fee: Some(Fee {
            recv_fee: get_fee_item(
                config.relay_fee.denom.clone(),
                config.relay_fee.amount.clone(),
            ),
            ack_fee: get_fee_item(
                config.relay_fee.denom.clone(),
                config.relay_fee.amount.clone(),
            ),
            timeout_fee: get_fee_item(
                config.relay_fee.denom.clone(),
                config.relay_fee.amount.clone(),
            ),
        }),
    };

    Ok(Response::new()
        .add_message(transfer_msg)
        .add_attribute("action", "ibc_transfer_to_ethereum"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
    }
}

fn handle_ibc_transfer_reply(_deps: DepsMut, _msg: Reply) -> Result<Response, ContractError> {
    // Here you can handle the reply from the IBC transfer.
    // For example, you could parse the reply data to get the sequence number
    // and store some information about the pending transfer.
    // For this example, we'll just log that the transfer was successful.
    Ok(Response::new().add_attribute("action", "ibc_transfer_reply_success"))
}

fn get_fee_item(denom: String, amount: Uint128) -> Vec<StdCoin> {
    if amount == Uint128::new(0) {
        vec![]
    } else {
        let coin = StdCoin {
            amount: amount.to_string(),
            denom,
        };
        vec![coin]
    }
}
