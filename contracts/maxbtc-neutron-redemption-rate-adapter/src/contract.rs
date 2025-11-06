use std::str::FromStr;
use cosmwasm_std::{attr, to_json_binary, Decimal, Deps};
use cosmwasm_std::{Binary, DepsMut, Env, MessageInfo, Response};
use cw_ownable::{get_ownership, update_ownership};
use crate::error::{ContractError, ContractResult};
use crate::msg::{
    ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, RedemptionRateResponse, UpdateConfig,
};
use crate::state::{Config, CONFIG};
use neutron_sdk::bindings::msg::NeutronMsg;
use neutron_sdk::bindings::query::NeutronQuery;

const CONTRACT_NAME: &str = concat!("crates.io:maxbtc-neutron__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn instantiate(
    deps: DepsMut<NeutronQuery>,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response<NeutronMsg>> {
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    cw_ownable::initialize_owner(deps.storage, deps.api, Some(msg.owner.as_ref()))?;
    assert!(msg.fee_bps <= 10_000);
    let config = &Config {
        fee_bps: msg.fee_bps,
        core_contract: deps.api.addr_validate(&msg.core_contract)?,
        denom: msg.denom,
    };
    CONFIG.save(deps.storage, config)?;
    Ok(Response::new()
        .add_attribute("instantiate", CONTRACT_NAME)
        .add_attribute("owner", msg.owner))
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn query(deps: Deps<NeutronQuery>, env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::Ownership {} => Ok(to_json_binary(&get_ownership(deps.storage)?)?),
        QueryMsg::Config {} => query_config(deps, env),
        QueryMsg::RedemptionRate { denom, .. } => query_redemption_rate(deps, env, denom),
    }
}

fn query_config(deps: Deps<NeutronQuery>, _env: Env) -> ContractResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    Ok(to_json_binary(&config)?)
}

fn query_redemption_rate(
    deps: Deps<NeutronQuery>,
    env: Env,
    denom: String,
) -> ContractResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    if denom != config.denom {
        return Err(ContractError::InvalidDenom {});
    }
    let fee_bps = Decimal::from_str(CONFIG.load(deps.storage)?.fee_bps.to_string().as_str())?;
    let exchange_rate: Decimal = deps.querier.query_wasm_smart(
        config.core_contract.clone(),
        &maxbtc_neutron_core::msg::QueryMsg::ExchangeRate {},
    )?;
    let redemption_rate = fee_bps / (Decimal::from_str("10_000")?) * exchange_rate;
    Ok(to_json_binary(&RedemptionRateResponse {
        redemption_rate,
        update_time: env.block.time.seconds(),
    })?)
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn execute(
    deps: DepsMut<NeutronQuery>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response<NeutronMsg>> {
    match msg {
        ExecuteMsg::UpdateOwnership(action) => {
            update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new())
        }
        ExecuteMsg::UpdateConfig { new_config } => execute_update_config(deps, info, new_config),
    }
}

fn execute_update_config(
    deps: DepsMut<NeutronQuery>,
    info: MessageInfo,
    new_config: UpdateConfig,
) -> ContractResult<Response<NeutronMsg>> {
    cw_ownable::assert_owner(deps.storage, &info.sender)?;
    let new_core_contract = deps.api.addr_validate(&new_config.core_contract)?;
    assert!(new_config.fee_bps <= 10_000);
    CONFIG.save(
        deps.storage,
        &Config {
            fee_bps: new_config.fee_bps,
            core_contract: new_core_contract,
            denom: new_config.denom.clone(),
        },
    )?;

    let attrs = vec![
        attr("core_contract", new_config.core_contract.to_string()),
        attr("denom", new_config.denom),
        attr("fee_bps", new_config.fee_bps.to_string()),
    ];
    Ok(Response::new()
        .add_attribute("update_config", CONTRACT_NAME).add_attributes(attrs))
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn migrate(
    deps: DepsMut<NeutronQuery>,
    _env: Env,
    _msg: MigrateMsg,
) -> ContractResult<Response<NeutronMsg>> {
    let version: semver::Version = CONTRACT_VERSION.parse()?;
    let storage_version: semver::Version =
        cw2::get_contract_version(deps.storage)?.version.parse()?;
    if storage_version < version {
        cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    }
    Ok(Response::new())
}