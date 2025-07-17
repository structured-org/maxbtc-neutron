use cosmwasm_std::{
    attr, entry_point, to_json_binary, Attribute, Binary, Deps, DepsMut, Env, Event, MessageInfo,
    Response, StdResult,
};

use crate::{
    error::ContractResult,
    msgs::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    state::{ConfigOptional, ZkmeVerifyQueryMsg, CONFIG},
};

const CONTRACT_NAME: &str = "crates.io:maxbtc-kyc-checker";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let attrs: Vec<Attribute> = vec![
        attr(
            "zkme_verify_upgradeable_contract",
            &msg.zkme_verify_upgradeable_contract,
        ),
        attr("cooperator_address", &msg.cooperator_address),
        attr("owner", &msg.owner),
    ];

    cw_ownable::initialize_owner(deps.storage, deps.api, Some(msg.owner.as_str()))?;
    let config = msg.into_config(deps.as_ref().into_empty())?;
    CONFIG.save(deps.storage, &config)?;

    Ok(response("instantiate", CONTRACT_NAME, attrs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::HasApproved { user } => query_has_approved(deps, env, user),
        QueryMsg::GetConfig {} => query_config(deps),
        QueryMsg::Ownership {} => {
            let ownership = cw_ownable::get_ownership(deps.storage)?;

            Ok(to_json_binary(&ownership)?)
        }
    }
}

pub fn query_has_approved(deps: Deps, _env: Env, user: String) -> ContractResult<Binary> {
    let config = CONFIG.load(deps.storage)?;

    let has_approved: bool = deps.querier.query_wasm_smart(
        config.zkme_verify_upgradeable_contract.clone(),
        &ZkmeVerifyQueryMsg::HasApproved {
            cooperator: config.cooperator_address.clone(),
            user,
        },
    )?;

    Ok(to_json_binary(&has_approved)?)
}

fn query_config(deps: Deps) -> ContractResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    Ok(to_json_binary(&config)?)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig { new_config } => {
            execute_update_config(deps, env, info, new_config)
        }
        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(response::<(&str, &str), _>(
                "execute-update-ownership",
                CONTRACT_NAME,
                [],
            ))
        }
    }
}

fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_config: ConfigOptional,
) -> ContractResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    let mut attrs = vec![attr("action", "update_config")];
    if let Some(zkme_verify_upgradeable_contract) = new_config.zkme_verify_upgradeable_contract {
        config.zkme_verify_upgradeable_contract =
            deps.api.addr_validate(&zkme_verify_upgradeable_contract)?;
        attrs.push(cosmwasm_std::attr(
            "zkme_verify_upgradeable_contract",
            zkme_verify_upgradeable_contract,
        ));
    }

    if let Some(cooperator_address) = new_config.cooperator_address {
        config.cooperator_address = cooperator_address.clone();
        attrs.push(cosmwasm_std::attr("cooperator_address", cooperator_address));
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(response("update_config", CONTRACT_NAME, attrs))
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::new())
}

pub fn response<A: Into<Attribute>, T>(
    ty: &str,
    contract_name: &str,
    attrs: impl IntoIterator<Item = A>,
) -> Response<T> {
    Response::<T>::new()
        .add_event(Event::new(format!("{contract_name}-{ty}")).add_attributes(attrs))
}
