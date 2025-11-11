use crate::error::{ContractError, ContractResult};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, RedemptionRateResponse, UpdateConfig};
use crate::state::{Config, CONFIG};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, QueryRequest,
    Response,
};
use cw2::set_contract_version;
use cw_ownable::{get_ownership, update_ownership};
use maxbtc_base::msg::core::{ExchangeRateProviderQueryMsg, GetTwaerResponse};
use std::str::FromStr;

const CONTRACT_NAME: &str = concat!("crates.io:maxbtc-neutron__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    cw_ownable::initialize_owner(deps.storage, deps.api, Some(msg.owner.as_ref()))?;
    assert!(msg.fee_bps <= 10_000);

    let config = &Config {
        fee_bps: msg.fee_bps,
        twaer_provider_contract: deps
            .api
            .addr_validate(&msg.twaer_provider_contract)?
            .to_string(),
        denom: msg.denom.clone(),
    };
    CONFIG.save(deps.storage, config)?;
    Ok(Response::new()
        .add_attribute("instantiate", CONTRACT_NAME)
        .add_attribute("fee_bps", msg.fee_bps.to_string())
        .add_attribute("twaer_provider_contract", msg.twaer_provider_contract)
        .add_attribute("denom", msg.denom)
        .add_attribute("owner", msg.owner))
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::Ownership {} => Ok(to_json_binary(&get_ownership(deps.storage)?)?),
        QueryMsg::Config {} => query_config(deps, env),
        QueryMsg::RedemptionRate { denom, .. } => query_redemption_rate(deps, denom),
    }
}

fn query_config(deps: Deps, _env: Env) -> ContractResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    Ok(to_json_binary(&config)?)
}

fn query_redemption_rate(deps: Deps, denom: String) -> ContractResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    if denom != config.denom {
        return Err(ContractError::InvalidDenom {});
    }
    let fee_bps = Decimal::from_str(config.fee_bps.to_string().as_str())?;
    let twaer: GetTwaerResponse =
        deps.querier
            .query(&QueryRequest::Wasm(cosmwasm_std::WasmQuery::Smart {
                contract_addr: config.twaer_provider_contract.to_string(),
                msg: to_json_binary(&ExchangeRateProviderQueryMsg::GetTwaer {})?,
            }))?;
    let ten_thousand = Decimal::from_str("10000")?;
    let redemption_rate = (ten_thousand - fee_bps) / ten_thousand * twaer.twaer;
    Ok(to_json_binary(&RedemptionRateResponse {
        redemption_rate,
        update_time: twaer.published_at,
    })?)
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::UpdateOwnership(action) => {
            update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new())
        }
        ExecuteMsg::UpdateConfig { new_config } => execute_update_config(deps, info, new_config),
    }
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_config: UpdateConfig,
) -> ContractResult<Response> {
    cw_ownable::assert_owner(deps.storage, &info.sender)?;
    let mut config = CONFIG.load(deps.storage)?;
    let mut res = Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender.to_string());
    if let Some(new_denom) = new_config.denom {
        assert!(new_denom.is_empty());
        config.denom = new_denom.clone();
        res = res.add_attribute("denom", new_denom);
    }
    if let Some(new_fee_bps) = new_config.fee_bps {
        assert!(new_fee_bps <= 10_000);
        config.fee_bps = new_fee_bps;
        res = res.add_attribute("fee_bps", new_fee_bps.to_string());
    }
    if let Some(new_twaer_provider_contract) = new_config.twaer_provider_contract {
        deps.api
            .addr_validate(new_twaer_provider_contract.as_str())?
            .to_string();
        config.twaer_provider_contract = new_twaer_provider_contract.clone();
        res = res.add_attribute("twaer_provider_contract", new_twaer_provider_contract);
    }
    CONFIG.save(deps.storage, &config)?;
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(
    deps: DepsMut,
    _env: Env,
    _msg: maxbtc_base::msg::core::MigrateMsg,
) -> Result<Response, ContractError> {
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
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    }
    Ok(Response::new())
}
