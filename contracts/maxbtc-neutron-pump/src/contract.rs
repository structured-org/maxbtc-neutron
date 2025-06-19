use crate::error::ContractError;
use crate::msg::{
    Action, DestCallback, EurekaFee, ExecuteMsg, IbcInfo, IbcTransfer, IbcTransferAction,
    InstantiateMsg, Memo, QueryMsg, Wasm, WasmMsg,
};
use crate::state::{Config, CONFIG};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use neutron_sdk::sudo::msg::{RequestPacket, SudoMsg};
use neutron_std::types::cosmos::base::v1beta1::Coin as StdCoin;
use neutron_std::types::neutron::feerefunder::Fee;
use neutron_std::types::neutron::transfer::MsgTransfer;

const CONTRACT_NAME: &str = "crates.io:maxbtc-neutron-pump";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const EUREKA_MEMO_ENCODING: &str = "application/x-solidity-abi";

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
        transfer_denom: msg.transfer_denom.clone(),
        to_chain_receiver: msg.receiver.clone(),
        recover_address: msg.recover_address.clone(),
        source_port: msg.source_port.clone(),
        eureka_source_channel: msg.eureka_source_channel.clone(),
        neutron_source_channel: msg.neutron_source_channel.clone(),
        to_chain_entry_contract_address: msg.to_chain_entry_contract_address.clone(),
        to_chain_callback_contract_address: msg.to_chain_callback_contract_address.clone(),
        max_fee: msg.max_fee.clone(),
        oracle_address,
        exact_out: msg.exact_out,
        relay_fee: msg.relay_fee.clone(),
    };
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("transfer_denom", msg.transfer_denom)
        .add_attribute("receiver", msg.receiver)
        .add_attribute("recover_address", msg.recover_address)
        .add_attribute("source_port", msg.source_port)
        .add_attribute("eureka_source_channel", msg.eureka_source_channel)
        .add_attribute("neutron_source_channel", msg.neutron_source_channel)
        .add_attribute(
            "to_chain_entry_contract_address",
            msg.to_chain_entry_contract_address,
        )
        .add_attribute(
            "to_chain_callback_contract_address",
            msg.to_chain_callback_contract_address,
        )
        .add_attribute("max_fee", msg.max_fee.to_string())
        .add_attribute("oracle_address", msg.oracle_address)
        .add_attribute("exact_out", msg.exact_out.to_string())
        .add_attribute("relay_fee", msg.relay_fee.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Push {
            amount,
            eureka_fee,
            oracle_entry_address,
            eureka_source_channel,
            oracle_callback_address,
            eureka_fee_timeout_nano,
            eureka_full_timeout_nano,
        } => execute_transfer(
            deps,
            env,
            info,
            amount,
            eureka_fee,
            oracle_entry_address,
            oracle_callback_address,
            eureka_source_channel,
            eureka_fee_timeout_nano,
            eureka_full_timeout_nano,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Coin,
    eureka_fee: EurekaFee,
    oracle_entry_address: String,
    oracle_callback_address: String,
    eureka_source_channel: String,
    eureka_fee_timeout_nano: u64,
    eureka_full_timeout_nano: u64,
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
        return Err(ContractError::OracleMismatch {
            value: oracle_entry_address,
        });
    }
    if oracle_callback_address != config.to_chain_callback_contract_address {
        return Err(ContractError::OracleMismatch {
            value: oracle_callback_address,
        });
    }
    if eureka_source_channel != config.eureka_source_channel {
        return Err(ContractError::OracleMismatch {
            value: eureka_source_channel,
        });
    }

    // Check if the transfer denom matches the config
    if amount.denom != config.transfer_denom {
        return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
            "Invalid transfer denom",
        )));
    }

    // Construct the callback memo for the transfer packet
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
                            encoding: EUREKA_MEMO_ENCODING.to_string(),
                            eureka_fee: EurekaFee {
                                coin: eureka_fee.coin.clone(),
                                receiver: eureka_fee.receiver,
                                timeout_timestamp: eureka_fee_timeout_nano,
                            },
                            memo: "".to_string(),
                            receiver: config.to_chain_receiver.clone(),
                            recover_address: config.recover_address.clone(),
                            source_channel: eureka_source_channel.clone(),
                        },
                    }),
                },
                exact_out: config.exact_out,
                // Expects unix seconds instead of unix nano for some reason
                timeout_timestamp: eureka_full_timeout_nano / 1_000_000_000,
            },
        },
    };

    let memo_str = serde_json_wasm::to_string(&memo).unwrap();

    // Construct the IBC Transfer message
    let transfer_msg = MsgTransfer {
        source_port: config.source_port,
        source_channel: config.neutron_source_channel.clone(),
        sender: env.contract.address.to_string(),
        // The initial receiver on the dest chain
        receiver: config.to_chain_entry_contract_address,
        token: Some(StdCoin {
            denom: amount.denom.clone(),
            amount: amount.amount.to_string(),
        }),
        timeout_height: None,
        // We set this equal to the fee timeout to make sure that the fee is never expired
        timeout_timestamp: eureka_fee_timeout_nano,
        memo: memo_str,
        // Will be soon deprecated
        fee: Some(Fee {
            recv_fee: get_fee_item(config.relay_fee.denom.clone(), config.relay_fee.amount),
            ack_fee: get_fee_item(config.relay_fee.denom.clone(), config.relay_fee.amount),
            timeout_fee: get_fee_item(config.relay_fee.denom.clone(), config.relay_fee.amount),
        }),
    };

    Ok(Response::new()
        .add_message(transfer_msg)
        .add_attribute("action", "ibc_transfer_to_ethereum")
        .add_attribute("sender", info.sender)
        .add_attribute("amount", amount.to_string())
        .add_attribute("eureka_fee_amount", eureka_fee.coin.to_string())
        .add_attribute("eureka_fee_receiver", config.to_chain_receiver)
        .add_attribute("oracle_entry_address", oracle_entry_address)
        .add_attribute("oracle_callback_address", oracle_callback_address)
        .add_attribute("source_channel", eureka_source_channel)
        .add_attribute(
            "eureka_fee_timeout_nano",
            eureka_fee_timeout_nano.to_string(),
        )
        .add_attribute(
            "eureka_full_timeout_nano",
            eureka_full_timeout_nano.to_string(),
        ))
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn sudo(deps: DepsMut, env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    match msg {
        SudoMsg::Response { request, data } => sudo_response(deps, env, request, data),
        SudoMsg::Error { request, details } => sudo_error(deps, env, request, details),
        SudoMsg::Timeout { request } => sudo_timeout(deps, env, request),
        _ => Ok(Response::default()),
    }
}

fn sudo_response(
    _deps: DepsMut,
    _env: Env,
    _request: RequestPacket,
    _data: Binary,
) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "sudo_response"))
}

fn sudo_timeout(
    _deps: DepsMut,
    _env: Env,
    _request: RequestPacket,
) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "sudo_timeout"))
}

fn sudo_error(
    _deps: DepsMut,
    _env: Env,
    _request: RequestPacket,
    _details: String,
) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "sudo_error"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
    }
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
