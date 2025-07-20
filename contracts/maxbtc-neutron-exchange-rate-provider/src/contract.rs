#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, Uint128,
};
use cw2::set_contract_version;
use cw_ownable::assert_owner;

use crate::error::{ContractError, ContractResult};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{AUM, EXCHANGE_RATE};

const CONTRACT_NAME: &str = "crates.io:maxbtc-neutron-allow-list";
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
    AUM.save(deps.storage, &Uint128::zero())?;
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
        ExecuteMsg::UpdateAUM { aum } => {
            assert_owner(deps.storage, &info.sender)?;
            AUM.save(deps.storage, &aum)?;
            Ok(Response::new()
                .add_attribute("action", "update_aum")
                .add_attribute("aum", aum.to_string()))
        }
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
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    Ok(match msg {
        QueryMsg::Owner {} => to_json_binary(
            &cw_ownable::get_ownership(deps.storage)?
                .owner
                .unwrap_or(cosmwasm_std::Addr::unchecked(""))
                .to_string(),
        )?,
        QueryMsg::ExchangeRate {} => to_json_binary(&EXCHANGE_RATE.load(deps.storage)?)?,
        QueryMsg::AUM {} => to_json_binary(&AUM.load(deps.storage).unwrap_or(Uint128::zero()))?,
    })
}
