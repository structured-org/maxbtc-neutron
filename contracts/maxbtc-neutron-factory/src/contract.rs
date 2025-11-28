use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, WaitosaurObserverInstantiateMsg,
};
use crate::state::{State, WaitosaurObserverConfig, STATE};
use cosmwasm_std::{
    entry_point, instantiate2_address, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, WasmMsg,
};
use cw2::set_contract_version;
use cw_ownable::initialize_owner;
use maxbtc_base::{
    msg::{
        core::InstantiateMsg as CoreInstantiateMsg,
        token::InstantiateMsg as TokenFactoryInstantiateMsg,
        waitosaur_holder::InstantiateMsg as WaitosaurHolderInstantiateMsg,
        withdrawal_manager::InstantiateMsg as WithdrawalManagerInstantiateMsg,
    },
    state::waitosaur_holder::Config as WaitosaurHolderConfig,
};
use maxbtc_neutron_allow_list::msg::InstantiateMsg as AllowListInstantiateMsg;
use maxbtc_neutron_exchange_rate_provider::msg::InstantiateMsg as ExchangeRateProviderInstantiateMsg;
use maxbtc_neutron_fee_collector::msg::InstantiateMsg as FeeCollectorInstantiateMsg;

const CONTRACT_NAME: &str = concat!("crates.io:structured-maxbtc__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    initialize_owner(deps.storage, deps.api, Some(&msg.owner))?;

    let canonical_creator = deps.api.addr_canonicalize(env.contract.address.as_str())?;
    let salt = msg.salt.as_bytes();

    // Get the checksum for the fee collector contract's code ID
    let fee_collector_code_info = deps
        .querier
        .query_wasm_code_info(msg.code_ids.fee_collector_contract_code_id)?;
    let fee_collector_checksum = fee_collector_code_info.checksum;
    let fee_collector_address = instantiate2_address(
        fee_collector_checksum.as_slice(),
        &canonical_creator, // The creator is this core contract
        salt,
    )
    .map_err(ContractError::Instantiate2Error)?;
    let fee_collector_contract = deps.api.addr_humanize(&fee_collector_address)?;

    let token_code_info = deps
        .querier
        .query_wasm_code_info(msg.code_ids.token_code_id)?;
    let token_checksum = token_code_info.checksum;
    let token_address = instantiate2_address(
        token_checksum.as_slice(),
        &canonical_creator, // The creator is this core contract
        salt,
    )
    .map_err(ContractError::Instantiate2Error)?;
    let token_contract = deps.api.addr_humanize(&token_address)?;

    let exchange_rate_provider_code_info = deps
        .querier
        .query_wasm_code_info(msg.code_ids.exchange_rate_provider_contract_code_id)?;
    let exchange_rate_provider_checksum = exchange_rate_provider_code_info.checksum;
    let exchange_rate_provider_address = instantiate2_address(
        exchange_rate_provider_checksum.as_slice(),
        &canonical_creator, // The creator is this core contract
        salt,
    )
    .map_err(ContractError::Instantiate2Error)?;
    let exchange_rate_provider_contract =
        deps.api.addr_humanize(&exchange_rate_provider_address)?;

    let allowlist_code_info = deps
        .querier
        .query_wasm_code_info(msg.code_ids.allowlist_contract_code_id)?;
    let allowlist_checksum = allowlist_code_info.checksum;
    let allowlist_address = instantiate2_address(
        allowlist_checksum.as_slice(),
        &canonical_creator, // The creator is this core contract
        salt,
    )
    .map_err(ContractError::Instantiate2Error)?;
    let allowlist_contract = deps.api.addr_humanize(&allowlist_address)?;

    let core_code_info = deps
        .querier
        .query_wasm_code_info(msg.code_ids.core_code_id)?;
    let core_checksum = core_code_info.checksum;
    let core_address = instantiate2_address(
        core_checksum.as_slice(),
        &canonical_creator, // The creator is this core contract
        salt,
    )
    .map_err(ContractError::Instantiate2Error)?;
    let core_contract = deps.api.addr_humanize(&core_address)?;

    let waitosaur_observer_code_info = deps
        .querier
        .query_wasm_code_info(msg.code_ids.waitosaur_observer_contract_code_id)?;
    let waitosaur_observer_checksum = waitosaur_observer_code_info.checksum;
    let waitosaur_observer_address = instantiate2_address(
        waitosaur_observer_checksum.as_slice(),
        &canonical_creator, // The creator is this core contract
        salt,
    )
    .map_err(ContractError::Instantiate2Error)?;
    let waitosaur_observer_contract = deps.api.addr_humanize(&waitosaur_observer_address)?;

    let waitosaur_holder_code_info = deps
        .querier
        .query_wasm_code_info(msg.code_ids.waitosaur_holder_contract_code_id)?;
    let waitosaur_holder_checksum = waitosaur_holder_code_info.checksum;
    let waitosaur_holder_address = instantiate2_address(
        waitosaur_holder_checksum.as_slice(),
        &canonical_creator, // The creator is this core contract
        salt,
    )
    .map_err(ContractError::Instantiate2Error)?;

    let waitosaur_holder_contract = deps.api.addr_humanize(&waitosaur_holder_address)?;

    let withdrawal_magnager_code_info = deps
        .querier
        .query_wasm_code_info(msg.code_ids.withdrawal_manager_contract_code_id)?;
    let withdrawal_magnager_checksum = withdrawal_magnager_code_info.checksum;
    let withdrawal_magnager_address = instantiate2_address(
        withdrawal_magnager_checksum.as_slice(),
        &canonical_creator, // The creator is this core contract
        salt,
    )
    .map_err(ContractError::Instantiate2Error)?;
    let withdrawal_manager_contract = deps.api.addr_humanize(&withdrawal_magnager_address)?;

    // Instantiate contracts messages
    let instantiate_withdrawal_manager_msg = WasmMsg::Instantiate2 {
        admin: Some(env.contract.address.to_string()), // The core contract owner is admin
        code_id: msg.code_ids.withdrawal_manager_contract_code_id,
        label: "maxBTC Withdrawal Manager Contract".to_string(),
        msg: to_json_binary(&WithdrawalManagerInstantiateMsg {
            owner: msg.owner.to_string(),
            factory_contract: env.contract.address.to_string(),
            core_contract: core_contract.to_string(),
            token_contract: token_contract.to_string(),
            deposit_denom: msg.deposit_denom.clone(),
        })?,
        funds: vec![],
        salt: Binary::from(salt),
    };

    let instantiate_waitosaur_observer_msg = WasmMsg::Instantiate2 {
        admin: Some(env.contract.address.to_string()), // The core contract owner is admin
        code_id: msg.code_ids.waitosaur_observer_contract_code_id,
        label: "maxBTC Waitosaur Contract".to_string(),
        msg: to_json_binary(&WaitosaurObserverInstantiateMsg {
            owner: msg.owner.to_string(),
            config: WaitosaurObserverConfig {
                locker: core_contract.clone(),
                unlocker: deps.api.addr_validate(&msg.waitosaur_observer_unlocker)?,
                contract: deps.api.addr_validate(&msg.binance_aum_contract)?,
                asset: msg.deposit_denom.clone(),
            },
        })?,
        funds: vec![],
        salt: Binary::from(salt),
    };

    let instantiate_allowlist_msg = WasmMsg::Instantiate2 {
        admin: Some(env.contract.address.to_string()), // The core contract owner is admin
        code_id: msg.code_ids.allowlist_contract_code_id,
        label: "maxBTC Allow List Contract".to_string(),
        msg: to_json_binary(&AllowListInstantiateMsg {
            owner: msg.owner.to_string(),
        })?,
        funds: vec![],
        salt: Binary::from(salt),
    };

    let instantiate_exchange_rate_provider_msg = WasmMsg::Instantiate2 {
        admin: Some(env.contract.address.to_string()), // The core contract owner is admin
        code_id: msg.code_ids.exchange_rate_provider_contract_code_id,
        label: "maxBTC Exchange Rate Provider Contract".to_string(),
        msg: to_json_binary(&ExchangeRateProviderInstantiateMsg {
            owner: msg.owner.to_string(), // Same owner as the core contract
        })?,
        funds: vec![],
        salt: Binary::from(salt),
    };

    let instantiate_fee_collector_msg = WasmMsg::Instantiate2 {
        admin: Some(env.contract.address.to_string()), // The core contract owner is admin
        code_id: msg.code_ids.fee_collector_contract_code_id,
        label: "maxBTC Fee Collector Contract".to_string(),
        msg: to_json_binary(&FeeCollectorInstantiateMsg {
            owner: msg.owner.to_string(), // Same owner as the core contract
            core_contract: core_contract.to_string(), // This contract's address
            fee_apy_reduction_percentage: msg.fee_collector_params.fee_apy_reduction_percentage,
            collection_period_seconds: msg.fee_collector_params.collection_period_seconds,
            fee_denom: get_maxbtc_denom(token_contract.to_string(), msg.maxbtc_denom.clone()),
            maxbtc_decimals: msg.deposit_decimals,
        })?,
        funds: vec![],
        salt: Binary::from(salt),
    };

    let instantiate_token_factory_msg = WasmMsg::Instantiate2 {
        admin: Some(env.contract.address.to_string()), // The core contract owner is admin
        code_id: msg.code_ids.token_code_id,
        label: "maxBTC Token Factory Contract".to_string(),
        msg: to_json_binary(&TokenFactoryInstantiateMsg {
            owner: core_contract.to_string(), // Same owner as the core contract
            factory_contract: env.contract.address.to_string(), // This contract's address
            subdenom: msg.maxbtc_denom.clone(),
        })?,
        funds: vec![],
        salt: Binary::from(salt),
    };

    let instantiate_waitosaur_holder_msg = WasmMsg::Instantiate2 {
        admin: Some(env.contract.address.to_string()), // The core contract owner is admin
        code_id: msg.code_ids.waitosaur_holder_contract_code_id,
        label: "maxBTC Waitosaur Holder Contract".to_string(),
        msg: to_json_binary(&WaitosaurHolderInstantiateMsg {
            owner: msg.owner.to_string(),
            config: WaitosaurHolderConfig {
                locker: deps.api.addr_validate(&msg.ceffu_backend)?,
                unlocker: core_contract.clone(),
                asset: msg.deposit_denom.clone(),
                withdraw_manager_contract: withdrawal_manager_contract.clone(),
            },
        })?,
        funds: vec![],
        salt: Binary::from(salt),
    };

    let instantiate_core_msg = WasmMsg::Instantiate2 {
        admin: Some(env.contract.address.to_string()), // The core contract owner is admin
        code_id: msg.code_ids.core_code_id,
        label: "maxBTC Core Contract".to_string(),
        msg: to_json_binary(&CoreInstantiateMsg {
            owner: msg.owner.to_string(),
            operator: msg.operator.to_string(),
            token_contract: token_contract.to_string(),
            factory_contract: env.contract.address.to_string(),
            deposit_forwarder_contract: msg.deposit_forwarder_contract.to_string(),
            deposit_denom: msg.deposit_denom.clone(),
            deposit_decimals: msg.deposit_decimals,
            deposit_cost: msg.deposit_cost,
            deposits_cap: msg.deposits_cap,
            allowlist_contract: allowlist_contract.to_string(),
            exchange_rate_provider_contract: exchange_rate_provider_contract.to_string(),
            fee_collector_contract: fee_collector_contract.to_string(),
            waitosaur_observer_contract: waitosaur_observer_contract.to_string(),
            waitosaur_holder_contract: waitosaur_holder_contract.to_string(),
            withdrawal_manager_contract: withdrawal_manager_contract.to_string(),
        })?,
        funds: vec![],
        salt: Binary::from(salt),
    };

    let state = State {
        allowlist_contract,
        exchange_rate_provider_contract,
        fee_collector_contract,
        token_contract,
        core_contract,
        waitosaur_observer_contract,
        waitosaur_holder_contract,
        withdrawal_manager_contract,
    };

    STATE.save(deps.storage, &state)?;

    // 5. Build the final response with all necessary messages and attributes
    Ok(Response::new()
        .add_message(instantiate_waitosaur_observer_msg)
        .add_message(instantiate_withdrawal_manager_msg)
        .add_message(instantiate_allowlist_msg)
        .add_message(instantiate_exchange_rate_provider_msg)
        .add_message(instantiate_token_factory_msg)
        .add_message(instantiate_waitosaur_holder_msg)
        .add_message(instantiate_core_msg)
        .add_message(instantiate_fee_collector_msg)
        .add_attribute("action", "instantiate")
        .add_attribute("owner", msg.owner))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new().add_attribute("action", "update_ownership"))
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::State {} => {
            let state = STATE.load(deps.storage)?;
            Ok(to_json_binary(&state)?)
        }

        QueryMsg::Ownership {} => Ok(to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
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

    Ok(Response::new())
}
/* -----------------------------------------------------------------------------------------------
/ HELPER FUNCTIONS BELOW
/ -----------------------------------------------------------------------------------------------*/
pub fn get_maxbtc_denom(contract_addr: String, subdenom: String) -> String {
    format!("factory/{contract_addr}/{subdenom}")
}
