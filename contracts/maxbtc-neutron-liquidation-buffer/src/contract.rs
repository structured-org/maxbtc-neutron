use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128,
};
use cw2::set_contract_version;

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
        owned_maxbtc: msg.owned_maxbtc.unwrap_or(Uint128::zero()),
        owned_btc: msg.owned_btc.unwrap_or(Uint128::zero()),
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
        ExecuteMsg::ClawBack { amount } => clawback(deps, env, info, amount),
    }
}

pub fn clawback(
    _deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Coin,
) -> Result<Response, ContractError> {
    // TODO: do not forget to make this authorized
    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![amount],
    });
    Ok(Response::default().add_message(msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetBTCBalance {} => {
            let config = CONFIG.load(deps.storage)?;
            to_json_binary(&config.owned_btc)
        }
        QueryMsg::GetMaxBTCBalance {} => {
            let config = CONFIG.load(deps.storage)?;
            to_json_binary(&config.owned_maxbtc)
        }
    }
}

#[cfg(test)]
mod tests {}
