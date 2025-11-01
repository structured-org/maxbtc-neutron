use crate::error::ContractError;
use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    Uint128,
};
use cw2::set_contract_version;
use cw_ownable::initialize_owner;
use maxbtc_base::msg::waitosaur_holder::{
    ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, UpdateConfig,
};
use maxbtc_base::state::waitosaur_holder::{State, CONFIG, STATE};

const CONTRACT_NAME: &str = concat!("crates.io:structured-maxbtc__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    initialize_owner(deps.storage, deps.api, Some(&msg.owner))?;

    deps.api.addr_validate(msg.config.locker.as_ref())?;
    deps.api.addr_validate(msg.config.unlocker.as_ref())?;
    deps.api
        .addr_validate(msg.config.withdraw_manager_contract.as_ref())?;
    CONFIG.save(deps.storage, &msg.config)?;
    STATE.save(deps.storage, &State::Unlocked {})?;

    // 5. Build the final response with all necessary messages and attributes
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("locker", msg.config.locker.to_string())
        .add_attribute("unlocker", msg.config.unlocker.to_string())
        .add_attribute(
            "withdraw_manager_contract",
            msg.config.withdraw_manager_contract.to_string(),
        )
        .add_attribute("asset", msg.config.asset.clone()))
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
        ExecuteMsg::UpdateConfig { new_config } => execute_update_config(deps, info, new_config),
        ExecuteMsg::Lock { amount } => execute_lock(deps, env, info, amount),
        ExecuteMsg::Unlock {} => execute_unlock(deps, env, info),
    }
}

fn execute_lock(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.locker && cw_ownable::assert_owner(deps.storage, &info.sender).is_err()
    {
        return Err(ContractError::Unauthorized {});
    }
    let current_state = STATE.load(deps.storage)?;
    if let State::Locked { .. } = current_state {
        return Err(ContractError::AlreadyLocked {});
    }
    let state = State::Locked {
        amount,
        at_timestamp: env.block.time.nanos(),
    };
    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attribute("action", "lock"))
}

fn execute_unlock(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.unlocker
        && cw_ownable::assert_owner(deps.storage, &info.sender).is_err()
    {
        return Err(ContractError::Unauthorized {});
    }
    let current_state = STATE.load(deps.storage)?;
    match current_state {
        State::Locked {
            amount,
            at_timestamp: _,
        } => {
            let waitosaur_balance = deps
                .querier
                .query_balance(env.contract.address, config.asset.as_str())?;

            if waitosaur_balance.amount < amount {
                return Err(ContractError::InsufficientAssetAmount {});
            }

            let msg = CosmosMsg::Bank(BankMsg::Send {
                to_address: config.withdraw_manager_contract.to_string(),
                amount: vec![waitosaur_balance],
            });

            let state = State::Unlocked {};
            STATE.save(deps.storage, &state)?;
            Ok(Response::new()
                .add_message(msg)
                .add_attribute("action", "unlock"))
        }
        State::Unlocked {} => Err(ContractError::AlreadyUnlocked {}),
    }
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_config: UpdateConfig,
) -> Result<Response, ContractError> {
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    let mut config = CONFIG.load(deps.storage)?;

    // Update configuration if fields are provided
    if let Some(locker) = new_config.locker {
        config.locker = deps.api.addr_validate(&locker)?;
    }
    if let Some(unlocker) = new_config.unlocker {
        config.unlocker = deps.api.addr_validate(&unlocker)?;
    }
    if let Some(asset) = new_config.asset {
        config.asset = asset;
    }
    if let Some(withdraw_manager_contract) = new_config.withdraw_manager_contract {
        config.withdraw_manager_contract = deps.api.addr_validate(&withdraw_manager_contract)?;
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<cosmwasm_std::Binary, ContractError> {
    match msg {
        QueryMsg::GetConfig {} => Ok(to_json_binary(&CONFIG.load(deps.storage)?)?),
        QueryMsg::GetState {} => Ok(to_json_binary(&STATE.load(deps.storage)?)?),
        QueryMsg::Ownership {} => Ok(to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?),
    }
}

/* -----------------------------------------------------------------------------------------------
/ HELPER FUNCTIONS BELOW
/ -----------------------------------------------------------------------------------------------*/
pub fn get_maxbtc_denom(contract_addr: String, subdenom: String) -> String {
    format!("factory/{contract_addr}/{subdenom}")
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}
