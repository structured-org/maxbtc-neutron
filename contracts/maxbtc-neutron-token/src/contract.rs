use crate::error::ContractError;
use cosmwasm_std::{
    entry_point, instantiate2_address, to_json_binary, Addr, BankMsg, Binary, Checksum,
    CodeInfoResponse, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_ownable::{assert_owner, initialize_owner};
use cw_storage_plus::Item;
use maxbtc_base::msg::{
    core::InstantiateMsg as CoreInstantiateMsg,
    token::{DenomMetadata, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    waitosaur_holder::InstantiateMsg as WaitosaurHolderInstantiateMsg,
    withdrawal_manager,
};
use maxbtc_base::state::{
    token::{Config, CONFIG},
    waitosaur_holder::Config as WaitosaurHolderConfig,
};
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
        ExecuteMsg::CreateRedemptionToken {
            redemption_subdenom,
        } => execute_create_redemption_token(deps, env, info, redemption_subdenom),
        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new().add_attribute("action", "update_ownership"))
        }
    }
}

fn execute_create_redemption_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    redemption_subdenom: String,
) -> Result<Response, ContractError> {
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    // Creates the redemption token factory denom
    let create_denom_msg =
        create_tokenfactory_create_denom_msg(&env.clone(), redemption_subdenom.to_string())?;

    // Return the response
    Ok(Response::new()
        .add_message(create_denom_msg)
        .add_attribute("action", "create_redemption_token")
        .add_attribute("sender", info.sender)
        .add_attribute("redemption_subdenom", redemption_subdenom))
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
    amount: Coin,
    recipient: String,
) -> Result<Response, ContractError> {
    cw_ownable::assert_owner(deps.storage, &info.sender)?;
    let cfg = CONFIG.load(deps.storage)?;

    // Mint the maxBTC to the recipient
    let mint_msg = create_tokenfactory_mint_msg(&env.clone(), recipient.clone(), amount.clone())?;

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
    _deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    if info.funds.len() != 1 {
        return Err(ContractError::WrongFundsAttached {});
    }
    let first_coin = &info
        .funds
        .first()
        .cloned()
        .ok_or(ContractError::WrongFundsAttached {})?;

    // Mint the maxBTC to the recipient
    let mint_msg = create_tokenfactory_burn_msg(&env.clone(), first_coin.clone())?;

    // Return the response
    Ok(Response::new()
        .add_message(mint_msg)
        .add_attribute("action", "burn")
        .add_attribute("sender", info.sender)
        .add_attribute("burned_amount", first_coin.to_string()))
}

pub(crate) fn execute_set_token_metadata(
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
        token_metadata.clone(),
    )?;

    Ok(Response::new()
        .add_message(metadata_msg)
        .add_attribute("action", "set_token_metadata")
        .add_attribute("sender", info.sender)
        .add_attribute("denom", cfg.denom.clone())
        .add_attribute("exponent", token_metadata.exponent.to_string())
        .add_attribute("display", token_metadata.display)
        .add_attribute("name", token_metadata.name)
        .add_attribute("description", token_metadata.description)
        .add_attribute("symbol", token_metadata.symbol)
        .add_attribute("uri", token_metadata.uri.unwrap_or_default())
        .add_attribute("uri_hash", token_metadata.uri_hash.unwrap_or_default()))
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<cosmwasm_std::Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => {
            let cfg = CONFIG.load(deps.storage)?;
            Ok(to_json_binary(&cfg)?)
        }
        QueryMsg::Ownership {} => Ok(to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?),
        QueryMsg::GetDenom { subdenom } => Ok(to_json_binary(&get_denom(deps, env, subdenom)?)?),
    }
}

pub fn get_denom(deps: Deps, env: Env, subdenom: Option<String>) -> Result<String, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if let Some(subdenom) = subdenom {
        Ok(get_maxbtc_denom(env.contract.address.to_string(), subdenom))
    } else {
        Ok(cfg.denom)
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version_metadata = cw2::get_contract_version(deps.storage)?;

    let storage_version: semver::Version = contract_version_metadata.version.parse()?;
    let version: semver::Version = CONTRACT_VERSION.parse()?;

    if storage_version < version {
        cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

        // Remove in next version
        // deps.storage.remove("last_deposit_flush_time".as_bytes());
        // deps.storage.remove("total_deposited".as_bytes());
        // deps.storage.remove("config".as_bytes()); // new config are stored under another key

        let salt = msg.salt.as_bytes();
        let canonical_self_address = deps.api.addr_canonicalize(env.contract.address.as_str())?;
        let core_contract_checksum = get_code_checksum(deps.as_ref(), msg.core_code_id)?;
        let core_address = instantiate2_address(
            core_contract_checksum.as_slice(),
            &canonical_self_address,
            salt,
        )?;
        let core_contract = deps.api.addr_humanize(&core_address)?;

        initialize_owner(deps.storage, deps.api, Some(core_contract.as_str()))?;

        let waitosaur_holder_code_info = deps
            .querier
            .query_wasm_code_info(msg.waitosaur_holder_code_id)?;
        let waitosaur_holder_checksum = waitosaur_holder_code_info.checksum;
        let waitosaur_holder_address = instantiate2_address(
            waitosaur_holder_checksum.as_slice(),
            &canonical_self_address,
            salt,
        )?;
        let waitosaur_holder_contract = deps.api.addr_humanize(&waitosaur_holder_address)?;

        let waitosaur_observer_code_info = deps
            .querier
            .query_wasm_code_info(msg.waitosaur_observer_code_id)?;
        let waitosaur_observer_checksum = waitosaur_observer_code_info.checksum;
        let waitosaur_observer_address = instantiate2_address(
            waitosaur_observer_checksum.as_slice(),
            &canonical_self_address,
            salt,
        )?;
        let waitosaur_observer_contract = deps.api.addr_humanize(&waitosaur_observer_address)?;

        let withdrawal_manager_code_info = deps
            .querier
            .query_wasm_code_info(msg.withdrawal_manager_code_id)?;
        let withdrawal_manager_checksum = withdrawal_manager_code_info.checksum;
        let withdrawal_manager_address = instantiate2_address(
            withdrawal_manager_checksum.as_slice(),
            &canonical_self_address,
            salt,
        )?;
        let withdrawal_manager_contract = deps.api.addr_humanize(&withdrawal_manager_address)?;

        #[cosmwasm_schema::cw_serde]
        pub struct OldConfig {
            pub paused: bool,
            pub deposit_forwarder_contract: Addr,
            pub deposit_denom: String,
            pub deposit_decimals: u32,
            pub maxbtc_denom: String,
            pub deposit_flush_period: u64,
            pub deposit_cost: Decimal,
            pub deposits_cap: Option<Uint128>,
            pub allowlist_contract: Addr,
            pub exchange_rate_provider_contract: Addr,
            pub fee_collector_contract: Addr,
        }

        let old_config = Item::<OldConfig>::new("config").load(deps.storage)?;

        #[cosmwasm_schema::cw_serde]
        pub struct WaitosaurObserverConfig {
            pub locker: Addr,
            pub unlocker: Addr,
            pub contract: Addr,
            pub asset: String,
        }

        #[cosmwasm_schema::cw_serde]
        pub struct WaitosaurObserverInstantiateMsg {
            pub config: WaitosaurObserverConfig,
            pub owner: String,
        }

        let instantiate_withdrawal_manager_contract_msg = CosmosMsg::Wasm(WasmMsg::Instantiate2 {
            admin: Some(msg.factory_contract.to_string()), // The core contract owner is admin
            code_id: msg.withdrawal_manager_code_id,
            label: "maxBTC Withdrawal Manager Contract".to_string(),
            msg: to_json_binary(&withdrawal_manager::InstantiateMsg {
                owner: msg.factory_contract.to_string(),
                factory_contract: msg.factory_contract.to_string(),
                core_contract: core_contract.to_string(),
                token_contract: env.contract.address.to_string(),
                deposit_denom: old_config.deposit_denom.clone(),
            })?,
            funds: vec![],
            salt: Binary::from(salt),
        });

        let instantiate_waitosaur_contract_msg = CosmosMsg::Wasm(WasmMsg::Instantiate2 {
            admin: Some(msg.factory_contract.to_string()), // The core contract owner is admin
            code_id: msg.waitosaur_observer_code_id,
            label: "maxBTC Waitosaur Contract".to_string(),
            msg: to_json_binary(&WaitosaurObserverInstantiateMsg {
                owner: msg.factory_contract.to_string(),
                config: WaitosaurObserverConfig {
                    locker: core_contract.clone(),
                    unlocker: deps.api.addr_validate(&msg.waitosaur_observer_unlocker)?,
                    contract: deps.api.addr_validate(&msg.binance_aum_contract)?,
                    asset: old_config.deposit_denom.clone(),
                },
            })?,
            funds: vec![],
            salt: Binary::from(salt),
        });

        let instantiate_waitosaur_holder_contract_msg = CosmosMsg::Wasm(WasmMsg::Instantiate2 {
            admin: Some(msg.factory_contract.to_string()), // The core contract owner is admin
            code_id: msg.waitosaur_holder_code_id,
            label: "maxBTC Withdrawal Notifier Contract".to_string(),
            msg: to_json_binary(&WaitosaurHolderInstantiateMsg {
                owner: msg.factory_contract.to_string(),
                config: WaitosaurHolderConfig {
                    locker: deps.api.addr_validate(msg.ceffu_backend.as_str())?,
                    unlocker: core_contract.clone(),
                    asset: old_config.deposit_denom.clone(),
                    withdraw_manager_contract: withdrawal_manager_contract.clone(),
                },
            })?,
            funds: vec![],
            salt: Binary::from(salt),
        });

        let instantiate_core_contract_msg = CosmosMsg::Wasm(WasmMsg::Instantiate2 {
            admin: Some(msg.factory_contract.to_string()), // The core contract owner is admin
            code_id: msg.core_code_id,
            label: "maxBTC Core Contract".to_string(),
            msg: to_json_binary(&CoreInstantiateMsg {
                owner: msg.factory_contract.to_string(),
                operator: msg.operator.to_string(),
                token_contract: env.contract.address.to_string(),
                factory_contract: msg.factory_contract.to_string(),
                deposit_forwarder_contract: old_config.deposit_forwarder_contract.into_string(),
                deposit_denom: old_config.deposit_denom,
                deposit_decimals: old_config.deposit_decimals,
                deposit_cost: old_config.deposit_cost,
                deposits_cap: old_config.deposits_cap,
                waitosaur_holder_contract: waitosaur_holder_contract.into_string(),
                allowlist_contract: old_config.allowlist_contract.into_string(),
                exchange_rate_provider_contract: old_config
                    .exchange_rate_provider_contract
                    .into_string(),
                fee_collector_contract: old_config.fee_collector_contract.into_string(),
                waitosaur_observer_contract: waitosaur_observer_contract.into_string(),
                withdrawal_manager_contract: withdrawal_manager_contract.into_string(),
            })?,
            funds: vec![],
            salt: Binary::from(salt),
        });

        let new_config = Config {
            factory_contract: msg.factory_contract,
            denom: get_maxbtc_denom(env.contract.address.to_string(), old_config.maxbtc_denom),
        };
        CONFIG.save(deps.storage, &new_config)?;

        let send_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: core_contract.to_string(),
            amount: vec![deposit_balance],
        });

        return Ok(Response::new()
            .add_message(instantiate_withdrawal_manager_contract_msg)
            .add_message(instantiate_waitosaur_contract_msg)
            .add_message(instantiate_waitosaur_holder_contract_msg)
            .add_message(instantiate_core_contract_msg)
            .add_message(send_msg)
            .add_attribute("core_contract", core_contract.to_string()));
    }
    Ok(Response::default())
}

/* -----------------------------------------------------------------------------------------------
/ HELPER FUNCTIONS BELOW
/ -----------------------------------------------------------------------------------------------*/
/// Formats the maxBTC denom for a given contract address
pub fn get_maxbtc_denom(contract_addr: String, subdenom: String) -> String {
    format!("factory/{contract_addr}/{subdenom}")
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

fn get_code_checksum(deps: Deps, code_id: u64) -> StdResult<Checksum> {
    let CodeInfoResponse { checksum, .. } = deps.querier.query_wasm_code_info(code_id)?;
    Ok(checksum)
}
