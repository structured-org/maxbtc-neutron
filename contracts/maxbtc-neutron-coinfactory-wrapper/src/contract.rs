use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG};
use cosmwasm_std::{
    entry_point, to_json_binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;
use cw_ownable::initialize_owner;
use maxbtc_base::msg::core::MigrateMsg;
use maxbtc_base::msg::token::{create_set_denom_metadata_msg, get_full_denom};
use neutron_std::types::cosmos::bank::v1beta1::MsgSend;
use neutron_std::types::cosmos::base::v1beta1::Coin;
use neutron_std::types::osmosis::tokenfactory::v1beta1::{MsgBurn, MsgCreateDenom, MsgMint};

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

    let full_denom = get_full_denom(env.contract.address.to_string(), msg.subdenom.clone());
    let cfg = Config {
        in_denom: msg.in_denom.clone(),
        out_denom: full_denom.clone(),
    };
    CONFIG.save(deps.storage, &cfg)?;

    let create_denom_submsg = Into::<CosmosMsg>::into(MsgCreateDenom {
        sender: env.contract.address.to_string(),
        subdenom: msg.subdenom.clone(),
    });
    let set_denom_metadata_submsg = create_set_denom_metadata_msg(
        env.contract.address.into_string(),
        full_denom.clone(),
        msg.token_metadata.clone(),
    )?;
    Ok(Response::new()
        .add_message(create_denom_submsg)
        .add_message(set_denom_metadata_submsg)
        .add_attribute("action", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("in_denom", msg.in_denom)
        .add_attribute("subdenom", msg.subdenom))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::Config {} => {
            let cfg = CONFIG.load(deps.storage)?;
            Ok(to_json_binary(&cfg)?)
        }
        QueryMsg::Denom {} => Ok(to_json_binary(&CONFIG.load(deps.storage)?.out_denom)?),
        QueryMsg::Ownership {} => Ok(to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?),
    }
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
        ExecuteMsg::Wrap {} => execute_wrap(deps, env, info),
        ExecuteMsg::Unwrap {} => execute_unwrap(deps, env, info),
    }
}

fn execute_unwrap(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    assert!(!config.out_denom.is_empty());

    let amount = cw_utils::must_pay(&info, config.out_denom.as_str())?;
    let burn_msg = Into::<CosmosMsg>::into(MsgBurn {
        sender: env.contract.address.to_string(),
        amount: Some(Coin {
            denom: config.out_denom,
            amount: amount.to_string(),
        }),
        burn_from_address: env.contract.address.to_string(),
    });
    let send_msg = Into::<CosmosMsg>::into(MsgSend {
        from_address: env.contract.address.to_string(),
        amount: vec![Coin {
            denom: config.in_denom,
            amount: amount.to_string(),
        }],
        to_address: info.sender.to_string(),
    });
    Ok(Response::new().add_messages(vec![burn_msg, send_msg]))
}

fn execute_wrap(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    assert!(!config.in_denom.is_empty());

    let amount = cw_utils::must_pay(&info, config.in_denom.as_str())?;
    let mint_msg = Into::<CosmosMsg>::into(MsgMint {
        sender: env.contract.address.to_string(),
        amount: Some(Coin {
            denom: config.out_denom,
            amount: amount.to_string(),
        }),
        mint_to_address: info.sender.to_string(),
    });
    Ok(Response::new().add_message(mint_msg))
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
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    }
    Ok(Response::new())
}
