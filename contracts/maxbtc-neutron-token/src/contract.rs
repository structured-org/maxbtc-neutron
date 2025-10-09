use crate::error::ContractError;
use crate::msg::{DenomMetadata, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{Config, CONFIG};
use cosmwasm_std::{
    entry_point, to_json_binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128,
};
use cw2::set_contract_version;
use cw_ownable::{assert_owner, initialize_owner};
use neutron_std::types::cosmos::bank::v1beta1::{DenomUnit, Metadata};
use neutron_std::types::cosmos::base::v1beta1::Coin as BaseCoin;
use neutron_std::types::osmosis::tokenfactory::v1beta1::{
    MsgBurn, MsgCreateDenom, MsgMint, MsgSetDenomMetadata,
};

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

    // Build the Config, now with the predictable fee collector address
    let cfg = Config {
        factory_contract: deps.api.addr_validate(&msg.factory_contract)?,
        denom: get_maxbtc_denom(env.contract.address.to_string(), msg.subdenom.clone()),
    };
    CONFIG.save(deps.storage, &cfg)?;

    // Create the maxBTC denom via token factory
    let create_maxbtc_denom_msg = create_tokenfactory_create_denom_msg(&env.clone(), msg.subdenom)?;

    // 5. Build the final response with all necessary messages and attributes
    Ok(Response::new()
        .add_message(create_maxbtc_denom_msg)
        .add_attribute("action", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("factory_contract", msg.factory_contract)
        .add_attribute("denom", cfg.denom.clone()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig { factory_contract } => {
            execute_update_config(deps, info, factory_contract)
        }
        ExecuteMsg::Mint { amount, recipient } => execute_mint(deps, env, info, amount, recipient),
        ExecuteMsg::Burn {} => execute_burn(deps, env, info),
        ExecuteMsg::SetTokenMetadata { token_metadata } => {
            execute_set_token_metadata(deps, env, info, token_metadata)
        }
        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new().add_attribute("action", "update_ownership"))
        }
    }
}

/// Owner-only handler that updates the configuration in-place.
fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    factory_address: Option<String>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;

    // Only the current owner may update the config.
    assert_owner(deps.storage, &info.sender)?;

    // Initialize the response with standard attributes.
    let mut res = Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender.to_string());

    if let Some(addr) = factory_address {
        let validated_addr = deps.api.addr_validate(&addr)?;
        cfg.factory_contract = validated_addr.clone();
        res = res.add_attribute("factory_contract", validated_addr.to_string());
    }

    // Save the updated configuration.
    CONFIG.save(deps.storage, &cfg)?;

    Ok(res)
}

pub(crate) fn execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    recipient: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // Can be equal to zero if rounding kicks in with a very high ER.
    if amount.is_zero() {
        return Err(ContractError::InvalidDepositAmount {});
    }

    // Mint the maxBTC to the recipient
    let mint_msg = create_tokenfactory_mint_msg(
        &env.clone(),
        recipient.clone(),
        Coin {
            amount,
            denom: cfg.denom.clone(),
        },
    )?;

    // Return the response
    Ok(Response::new()
        .add_message(mint_msg)
        .add_attribute("action", "mint")
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", recipient)
        .add_attribute("amount", amount.to_string())
        .add_attribute("denom", cfg.denom))
}

pub(crate) fn execute_burn(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let amount = cw_utils::must_pay(&info, &cfg.denom.clone())?;

    // Mint the maxBTC to the recipient
    let mint_msg = create_tokenfactory_burn_msg(
        &env.clone(),
        Coin {
            amount,
            denom: cfg.denom.clone(),
        },
    )?;

    // Return the response
    Ok(Response::new()
        .add_message(mint_msg)
        .add_attribute("action", "burn")
        .add_attribute("sender", info.sender)
        .add_attribute("amount", amount.to_string())
        .add_attribute("denom", cfg.denom))
}

fn execute_set_token_metadata(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_metadata: DenomMetadata,
) -> Result<Response, ContractError> {
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    let cfg = CONFIG.load(deps.storage)?;

    let metadata_msg = create_set_denom_metadata_msg(
        env.contract.address.into_string(),
        cfg.denom.clone(),
        token_metadata,
    )?;

    Ok(Response::new()
        .add_message(metadata_msg)
        .add_attribute("action", "set_token_metadata")
        .add_attribute("sender", info.sender)
        .add_attribute("denom", cfg.denom.clone()))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::Config {} => {
            let cfg = CONFIG.load(deps.storage)?;
            Ok(to_json_binary(&cfg)?)
        }
        QueryMsg::Ownership {} => Ok(to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?),
    }
}

/* -----------------------------------------------------------------------------------------------
/ HELPER FUNCTIONS BELOW
/ -----------------------------------------------------------------------------------------------*/
/// Formats the maxBTC denom for a given contract address
pub fn get_maxbtc_denom(contract_addr: String, subdenom: String) -> String {
    format!("factory/{}/{}", contract_addr, subdenom)
}

/// Creates a message to mint tokenfactory tokens of `denom` and credit them to `recipient`.
fn create_tokenfactory_mint_msg(
    env: &Env,
    recipient: String,
    amount: Coin,
) -> StdResult<CosmosMsg> {
    Ok(Into::<CosmosMsg>::into(MsgMint {
        sender: env.contract.address.to_string(),
        amount: Some(BaseCoin::from(amount)),
        mint_to_address: recipient,
    }))
}

/// Creates a message to mint tokenfactory tokens of `denom` and credit them to `recipient`.
fn create_tokenfactory_burn_msg(env: &Env, amount: Coin) -> StdResult<CosmosMsg> {
    Ok(Into::<CosmosMsg>::into(MsgBurn {
        sender: env.contract.address.to_string(),
        amount: Some(BaseCoin::from(amount)),
        burn_from_address: env.contract.address.to_string(),
    }))
}

/// Creates a message to create a tokenfactory denom.
fn create_tokenfactory_create_denom_msg(env: &Env, denom: String) -> StdResult<CosmosMsg> {
    Ok(Into::<CosmosMsg>::into(MsgCreateDenom {
        sender: env.contract.address.to_string(),
        subdenom: denom,
    }))
}

fn create_set_denom_metadata_msg(
    contract_address: String,
    denom: String,
    token_metadata: DenomMetadata,
) -> StdResult<CosmosMsg> {
    Ok(Into::<CosmosMsg>::into(MsgSetDenomMetadata {
        sender: contract_address.to_string(),
        metadata: Some(Metadata {
            denom_units: vec![
                DenomUnit {
                    denom: denom.clone(),
                    exponent: 0,
                    aliases: vec![],
                },
                DenomUnit {
                    denom: token_metadata.display.clone(),
                    exponent: token_metadata.exponent,
                    aliases: vec![],
                },
            ],
            base: denom,
            display: token_metadata.display,
            name: token_metadata.name,
            description: token_metadata.description,
            symbol: token_metadata.symbol,
            uri: token_metadata.uri.unwrap_or_default(),
            uri_hash: token_metadata.uri_hash.unwrap_or_default(),
        }),
    }))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}
