#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;
use cw_storage_plus::Bound;

use crate::error::{ContractError, ContractResult};
use crate::msg::{
    ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, ZkMeHasApprovedResponse, ZkMeQueryMsg,
    ZkMeSettings,
};
use crate::state::{ALLOW_LIST, ALLOW_LIST_V1, ZK_ME_SETTINGS};

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
        ExecuteMsg::Allow { addresses } => {
            cw_ownable::assert_owner(deps.storage, &info.sender)?;
            let mut attrs = vec![];
            for addr in addresses {
                let validated_addr = deps.api.addr_validate(&addr)?;
                ALLOW_LIST.save(deps.storage, &validated_addr, &())?;
                attrs.push(("allowed_address", validated_addr));
            }
            Ok(Response::new()
                .add_attribute("action", "allow_addresses")
                .add_attributes(attrs))
        }

        ExecuteMsg::Deny { addresses } => {
            cw_ownable::assert_owner(deps.storage, &info.sender)?;
            let mut attrs = vec![];
            for addr in addresses {
                let validated_addr = deps.api.addr_validate(&addr)?;
                ALLOW_LIST.remove(deps.storage, &validated_addr);
                attrs.push(("denied_address", validated_addr));
            }
            Ok(Response::new()
                .add_attribute("action", "deny_addresses")
                .add_attributes(attrs))
        }

        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new().add_attribute("action", "update_ownership"))
        }
        ExecuteMsg::UpdateZkMeSettings { settings } => {
            cw_ownable::assert_owner(deps.storage, &info.sender)?;
            if let Some(settings) = settings {
                ZK_ME_SETTINGS.save(
                    deps.storage,
                    &ZkMeSettings {
                        contract: deps.api.addr_validate(&settings.contract)?,
                        cooperator: deps.api.addr_validate(&settings.cooperator)?,
                    },
                )?;
            } else {
                ZK_ME_SETTINGS.remove(deps.storage);
            }
            Ok(Response::new().add_attribute("action", "update_zk_me_settings"))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    Ok(match msg {
        QueryMsg::Ownership {} => to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?,
        QueryMsg::AllowList { limit, start_after } => {
            use cosmwasm_std::Order;
            let limit = limit.unwrap_or(10).min(100) as usize;
            let start = start_after
                .map(|addr| deps.api.addr_validate(&addr))
                .transpose()?;
            let start = start.as_ref().map(Bound::exclusive);
            let addrs: Vec<String> = ALLOW_LIST
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit)
                .map(|item| item.map(|(addr, _)| addr.to_string()))
                .collect::<Result<_, _>>()?;
            to_json_binary(&addrs)?
        }
        QueryMsg::IsAddressAllowed { address } => {
            let is_allowed =
                ALLOW_LIST.may_load(deps.storage, &deps.api.addr_validate(&address)?)?;
            if is_allowed.is_some() {
                return to_json_binary(&true).map_err(ContractError::Std);
            }
            let zk_me_settings = ZK_ME_SETTINGS.may_load(deps.storage)?;
            if let Some(zk_me_settings) = zk_me_settings {
                let res: ZkMeHasApprovedResponse = deps.querier.query_wasm_smart(
                    zk_me_settings.contract,
                    &ZkMeQueryMsg::HasApproved {
                        user: deps.api.addr_validate(&address)?,
                        cooperator: zk_me_settings.cooperator,
                    },
                )?;
                return to_json_binary(&res.has_approved).map_err(ContractError::Std);
            } else {
                to_json_binary(&false)?
            }
        }
    })
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version_metadata = cw2::get_contract_version(deps.storage)?;
    let storage_contract_name = contract_version_metadata.contract.as_str();
    if storage_contract_name != CONTRACT_NAME {
        return Err(ContractError::MigrationError {
            storage_contract_name: storage_contract_name.to_string(),
            contract_name: CONTRACT_NAME.to_string(),
        });
    }

    let storage_version: semver::Version = contract_version_metadata.version.parse()?;
    let version: semver::Version = CONTRACT_VERSION.parse()?;

    if storage_version < version {
        cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    }

    if storage_version < semver::Version::parse("0.3.0")? {
        let old_list = ALLOW_LIST_V1.may_load(deps.storage)?;
        if let Some(old_list) = old_list {
            for addr in old_list {
                ALLOW_LIST.save(deps.storage, &addr, &())?;
            }
        }
        ALLOW_LIST_V1.remove(deps.storage);
    }

    Ok(Response::new())
}
