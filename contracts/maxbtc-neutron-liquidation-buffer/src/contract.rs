use crate::core_query::CoreQueryMsg;
use crate::error::ContractError;
use crate::helpers::*;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, UpdateConfig};
use crate::state::{Config, CONFIG};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps,
    DepsMut, Env, MessageInfo, QueryRequest, Response, StdResult, Uint128, WasmQuery,
};
use cw2::set_contract_version;

use neutron_std::types::neutron::dex::{LimitOrderType, MsgPlaceLimitOrder};

const CONTRACT_NAME: &str = "crates.io:maxbtc-neutron-liquidation-buffer";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        core_contract_address: msg.core_contract_address.clone(),
        price_multiplier_basis_points: msg.price_multiplier_basis_points,
        btc_denom: msg.btc_denom,
        maxbtc_denom: msg.maxbtc_denom,
        authority: msg.authority,
        treasury: msg.treasury,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Only core contract can call ClawBack.
        // returns the specified amount of BTC to the core contract via the following sequence:
        // [withdraw active order(s)] -> [send BTC to core contract] -> [re-place limit order with new amount]
        ExecuteMsg::ClawBack { amount } => clawback(deps, env, info, amount),
        // Only authority address can call this.
        // updates the config
        ExecuteMsg::UpdateConfig { config } => update_config(deps, env, info, config),
        // runs the sequence of actions:
        // [withdraw active order(s)] -> [burn maxBTC] -> [send maxBTC to treasury] -> [place new limit order]
        ExecuteMsg::RunSequence {} => run_sequence(deps, env, info),
    }
}

pub fn run_sequence(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> Result<Response, ContractError> {
    // Load the config
    let config = CONFIG.load(deps.storage)?;

    // construct message vector of all messages to be executed in sequence
    let mut messages = vec![];

    // Query the exchange rate from the core contract
    let exchange_rate: Decimal = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.core_contract_address,
        msg: to_json_binary(&CoreQueryMsg::ExchangeRate {})?,
    }))?;

    // get the deposit price modified by the discount rate
    let deposit_price = get_deposit_price(exchange_rate, config.price_multiplier_basis_points)?;

    // get the cancel messages for all limit orders, and the total tokens the contract will hold after canceling.
    // The expected tokens after canceling are added to the contract balance. we call the balances virtual
    // since the contract doesn't hold the tokens at this moment but will after the cancel messages executes.
    let (cancel_msgs, virtual_btc, virtual_max_btc) = get_cancel_messages(
        env.clone(),
        deps.as_ref(),
        config.btc_denom.clone(),
        config.maxbtc_denom.clone(),
    )?;

    // first append the cancel messages to the messages vector
    messages.extend(cancel_msgs);

    let mut burn_and_refund_messages = vec![];
    if virtual_max_btc > Uint128::zero() {
        burn_and_refund_messages = get_burn_and_refund_messages(
            env.clone(),
            virtual_max_btc,
            exchange_rate,
            deposit_price,
            config.treasury,
            config.maxbtc_denom.clone(),
        )?;
    }

    // then append the burn and refund messages to the messages vector
    messages.extend(burn_and_refund_messages);

    let dex_msg = Into::<CosmosMsg>::into(MsgPlaceLimitOrder {
        creator: env.contract.address.to_string(),
        receiver: env.contract.address.to_string(),
        token_in: config.btc_denom.clone(),
        token_out: config.maxbtc_denom.clone(),
        tick_index_in_to_out: 0,
        amount_in: virtual_btc.to_string(),
        order_type: LimitOrderType::GoodTilCancelled.into(),
        expiration_time: None,
        max_amount_out: None,
        limit_sell_price: Some(deposit_price.to_string()),
        min_average_sell_price: None,
    });

    // finally append the dex msg to the messages vector to place a new limit order.
    messages.push(dex_msg);

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "dex_deposit")
        .add_attribute("exchange_rate", exchange_rate.to_string()))
}

pub fn clawback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Coin,
) -> Result<Response, ContractError> {
    // Load config to get the core contract address
    let config = CONFIG.load(deps.storage)?;

    // Only allow the core contract to call this function
    if info.sender.to_string() != config.core_contract_address {
        return Err(ContractError::Unauthorized {});
    }

    let mut messages = vec![];

    let (cancel_msgs, virtual_btc, virtual_max_btc) = get_cancel_messages(
        env.clone(),
        deps.as_ref(),
        config.btc_denom.clone(),
        config.maxbtc_denom.clone(),
    )?;

    // first append the cancel messages to the messages vector
    messages.extend(cancel_msgs);

    // Send the specified amount of BTC back to the core contract
    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: config.core_contract_address,
        amount: vec![amount.clone()],
    });

    let remaining_btc = virtual_btc.checked_sub(amount.amount).unwrap_or_default();

    let dex_msg = Into::<CosmosMsg>::into(MsgPlaceLimitOrder {
        creator: env.contract.address.to_string(),
        receiver: env.contract.address.to_string(),
        token_in: config.btc_denom.clone(),
        token_out: config.maxbtc_denom.clone(),
        tick_index_in_to_out: 0,
        amount_in: virtual_btc.to_string(),
        order_type: LimitOrderType::GoodTilCancelled.into(),
        expiration_time: None,
        max_amount_out: None,
        limit_sell_price: Some(remaining_btc.to_string()),
        min_average_sell_price: None,
    });

    // finally append the dex msg to the messages vector to place a new limit order.
    messages.push(dex_msg);

    Ok(Response::default()
        .add_message(msg)
        .add_attribute("action", "clawback")
        .add_attribute("amount", amount.amount.to_string())
        .add_attribute("denom", amount.denom))
}

pub fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    config_msg: UpdateConfig,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    // Only allow the core contract to update configuration
    if info.sender.to_string() != config.authority {
        return Err(ContractError::Unauthorized {});
    }
    if let Some(new_address) = config_msg.core_contract_address {
        config.core_contract_address = new_address;
    }
    if let Some(new_btc_denom) = config_msg.btc_denom {
        config.btc_denom = new_btc_denom;
    }
    if let Some(new_maxbtc_denom) = config_msg.maxbtc_denom {
        config.maxbtc_denom = new_maxbtc_denom;
    }
    if let Some(new_price_multiplier_basis_points) = config_msg.price_multiplier_basis_points {
        config.price_multiplier_basis_points = new_price_multiplier_basis_points;
    }
    if let Some(new_authority) = config_msg.authority {
        config.authority = new_authority;
    }
    if let Some(new_treasury) = config_msg.treasury {
        config.treasury = new_treasury;
    }
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetBTCBalance {} => {
            let balances = get_virtual_contract_balance(
                _env.clone(),
                deps,
                CONFIG.load(deps.storage)?.btc_denom,
                CONFIG.load(deps.storage)?.maxbtc_denom,
            )
            .map_err(|e| cosmwasm_std::StdError::generic_err(e.to_string()))?;
            to_json_binary(&balances.0)
        }
        QueryMsg::GetMaxBTCBalance {} => {
            let balances = get_virtual_contract_balance(
                _env.clone(),
                deps,
                CONFIG.load(deps.storage)?.btc_denom,
                CONFIG.load(deps.storage)?.maxbtc_denom,
            )
            .map_err(|e| cosmwasm_std::StdError::generic_err(e.to_string()))?;
            to_json_binary(&balances.1)
        }
    }
}

#[cfg(test)]
mod tests {}
