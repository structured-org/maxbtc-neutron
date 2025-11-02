#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;
use cw_ownable::assert_owner;

use crate::error::{ContractError, ContractResult};
use crate::msg::{ExecuteMsg, GetTwaerResponse, InstantiateMsg, QueryMsg};
use crate::state::EXCHANGE_RATE;

const CONTRACT_NAME: &str = "crates.io:maxbtc-neutron-exchange-rate-provider";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    cw_ownable::initialize_owner(deps.storage, deps.api, Some(&msg.owner))?;
    EXCHANGE_RATE.save(deps.storage, &Decimal::one())?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::UpdateExchangeRate { rate } => {
            assert_owner(deps.storage, &info.sender)?;
            EXCHANGE_RATE.save(deps.storage, &rate)?;
            Ok(Response::new()
                .add_attribute("action", "update_exchange_rate")
                .add_attribute("rate", rate.to_string()))
        }

        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new()
                .add_attribute("action", "update_ownership")
                .add_attribute("new_owner", info.sender))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    Ok(match msg {
        QueryMsg::Ownership {} => to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?,
        QueryMsg::GetTwaer {} => to_json_binary(&GetTwaerResponse {
            twaer: EXCHANGE_RATE.load(deps.storage)?,
            published_at: env.block.time.seconds(),
        })?,
    })
}
