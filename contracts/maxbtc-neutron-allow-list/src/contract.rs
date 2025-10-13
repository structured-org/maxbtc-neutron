#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;

use crate::error::{ContractError, ContractResult};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::ALLOW_LIST;

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
    ALLOW_LIST.save(deps.storage, &vec![])?;
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
        ExecuteMsg::UpdateAllowList { allow_list } => {
            cw_ownable::assert_owner(deps.storage, &info.sender)?;
            let addresses: Result<Vec<_>, _> = allow_list
                .into_iter()
                .map(|addr| deps.api.addr_validate(&addr))
                .collect();
            let validated_addresses = addresses?;
            ALLOW_LIST.save(deps.storage, &validated_addresses)?;
            Ok(Response::new()
                .add_attribute("action", "update_allow_list")
                .add_attribute("allow_list", format!("{validated_addresses:?}")))
        }
        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new().add_attribute("action", "update_ownership"))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    Ok(match msg {
        QueryMsg::Ownership {} => to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?,
        QueryMsg::AllowList {} => {
            let allow_list = ALLOW_LIST.load(deps.storage)?;
            to_json_binary(
                &allow_list
                    .iter()
                    .map(|addr| addr.to_string())
                    .collect::<Vec<_>>(),
            )?
        }
        QueryMsg::IsAddressAllowed { address } => {
            let allow_list = ALLOW_LIST.load(deps.storage)?;
            let is_allowed_by_allow_list =
                allow_list.iter().any(|addr| addr.to_string() == address);
            to_json_binary(&is_allowed_by_allow_list)?
        }
    })
}
