use cosmwasm_std::{
    attr, to_json_binary, Attribute, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdResult, Uint128, WasmMsg,
};
use cw_ownable::{get_ownership, update_ownership};
use maxbtc_base::{
    msg::{
        core::QueryMsg as CoreQueryMsg,
        token::ExecuteMsg as TokenExecuteMsg,
        withdrawal_manager::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    },
    state::{
        core::Batch,
        withdrawal_manager::{Config, Pause, CONFIG, PAID_AMOUNT, PAUSE},
    },
};

use crate::error::ContractError;
const CONTRACT_NAME: &str = concat!("crates.io:structured-maxbtc__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    cw_ownable::initialize_owner(deps.storage, deps.api, Some(msg.owner.as_ref()))?;

    let attrs: Vec<Attribute> = vec![
        attr("action", "instantiate"),
        attr("factory_contract", &msg.factory_contract),
        attr("core_contract", &msg.core_contract),
        attr("token_contract", &msg.token_contract),
        attr("deposit_denom", &msg.deposit_denom),
    ];
    PAUSE.save(deps.storage, &Pause::default())?;
    CONFIG.save(
        deps.storage,
        &Config {
            factory_contract: deps.api.addr_validate(&msg.factory_contract)?,
            core_contract: deps.api.addr_validate(&msg.core_contract)?,
            token_contract: deps.api.addr_validate(&msg.token_contract)?,
            deposit_denom: msg.deposit_denom,
        },
    )?;
    Ok(Response::new().add_attributes(attrs))
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Ownership {} => Ok(to_json_binary(&get_ownership(deps.storage)?)?),
        QueryMsg::Config {} => Ok(to_json_binary(&CONFIG.load(deps.storage)?)?),
        QueryMsg::Pause {} => Ok(to_json_binary(&PAUSE.load(deps.storage)?)?),
    }
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateOwnership(action) => {
            update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new())
        }
        ExecuteMsg::UpdateConfig {
            factory_contract,
            core_contract,
            token_contract,
            deposit_denom,
        } => execute_update_config(
            deps,
            info,
            factory_contract,
            core_contract,
            token_contract,
            deposit_denom,
        ),
        ExecuteMsg::SetPause { pause } => execute_set_pause(deps, info, pause),
        ExecuteMsg::Claim { recipient } => execute_claim(deps, env, info, recipient),
    }
}

fn execute_set_pause(
    deps: DepsMut,
    info: MessageInfo,
    pause: Pause,
) -> Result<Response, ContractError> {
    cw_ownable::assert_owner(deps.storage, &info.sender)?;
    PAUSE.save(deps.storage, &pause)?;
    let attrs = vec![(
        "receive_nft_withdraw",
        pause.receive_nft_withdraw.to_string(),
    )];
    Ok(Response::new().add_attributes(attrs))
}

/// User claims their BTC, providing redemption tokens as input
pub(crate) fn execute_claim(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let pause = PAUSE.load(deps.storage)?;
    if pause.receive_nft_withdraw {
        return Err(ContractError::ContractPaused {});
    }

    let cfg = CONFIG.load(deps.storage)?;

    let mut msgs = vec![];

    if info.funds.len() != 1 {
        return Err(ContractError::WrongRedemptionTokenOrNoFunds {});
    }

    deps.api.addr_validate(&recipient)?;

    let redemption_coin = &info
        .funds
        .first()
        .cloned()
        .ok_or(ContractError::WrongRedemptionTokenOrNoFunds {})?;

    let batch_id = get_batch_id_from_redemption_coin(&cfg, redemption_coin.clone())?;

    // Check the batch is in FINALIZED state
    let batch: Batch = deps
        .querier
        .query(&QueryRequest::Wasm(cosmwasm_std::WasmQuery::Smart {
            contract_addr: cfg.core_contract.to_string(),
            msg: to_json_binary(&CoreQueryMsg::FinalizedBatch { batch_id })?,
        }))
        .map_err(|_| ContractError::BatchIsNotWithdrawn {})?;

    // The user’s portion = collected_amount * (user_redemption_tokens / total_redemption_tokens)
    let redemption_token_supply = deps.querier.query_supply(redemption_coin.denom.clone())?;
    if redemption_token_supply.amount.is_zero() {
        return Err(ContractError::RedemptionSupplyMismatch {});
    }

    let mut batch_paid_amount = PAID_AMOUNT.load(deps.storage, batch_id).unwrap_or_default();

    let user_amount_dec = Decimal::from_atomics(redemption_coin.amount, batch.deposit_decimals)?;
    let redemption_token_supply_dec =
        Decimal::from_atomics(redemption_token_supply.amount, batch.deposit_decimals)?;
    let fraction = user_amount_dec / redemption_token_supply_dec;

    let available_dec = Decimal::from_atomics(
        batch.collected_amount - batch_paid_amount,
        batch.deposit_decimals,
    )?;

    let user_btc = dec_to_amount(available_dec * fraction, batch.deposit_decimals)?;

    // Send the user’s BTC to `recipient` address
    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: recipient.clone(),
        amount: vec![Coin {
            denom: cfg.deposit_denom.clone(),
            amount: user_btc,
        }],
    });
    msgs.push(send_msg);

    // Burn the amount of redemption tokens that were redeemed
    let burn_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.token_contract.to_string(),
        msg: to_json_binary(&TokenExecuteMsg::Burn {})?,
        funds: vec![redemption_coin.clone()],
    });

    msgs.push(burn_msg);

    // Update the paid out amount in the batch
    batch_paid_amount += user_btc;
    PAID_AMOUNT.save(deps.storage, batch.batch_id, &batch_paid_amount)?;

    let resp = Response::new()
        .add_messages(msgs)
        .add_attribute("action", "claim")
        .add_attribute("batch_id", batch_id.to_string())
        .add_attribute("user_claim_btc", user_btc.to_string());

    Ok(resp)
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    factory_contract: Option<String>,
    core_contract: Option<String>,
    token_contract: Option<String>,
    deposit_denom: Option<String>,
) -> Result<Response, ContractError> {
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    let mut config = CONFIG.load(deps.storage)?;
    let mut attrs: Vec<Attribute> = vec![attr("action", "update_config")];

    if let Some(factory_contract) = factory_contract {
        config.factory_contract = deps.api.addr_validate(&factory_contract)?;
        attrs.push(attr("factory_contract", factory_contract));
    }
    if let Some(core_contract) = core_contract {
        config.core_contract = deps.api.addr_validate(&core_contract)?;
        attrs.push(attr("core_contract", core_contract));
    }
    if let Some(token_contract) = token_contract {
        config.token_contract = deps.api.addr_validate(&token_contract)?;
        attrs.push(attr("token_contract", token_contract));
    }
    if let Some(deposit_denom) = deposit_denom {
        attrs.push(attr("deposit_denom", &deposit_denom));
        config.deposit_denom = deposit_denom;
    }
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attributes(attrs))
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

    Ok(Response::new())
}

/// Extracts the `batch_id` encoded in the *redemption token*’s denom.
///
/// A valid redemption denom has the layout
/// `factory/{token_contract}/redemption/batch/{batch_id}`
fn get_batch_id_from_redemption_coin(
    cfg: &Config,
    redemption_coin: Coin,
) -> Result<u64, ContractError> {
    if !redemption_coin
        .denom
        .starts_with(format!("factory/{}/redemption/batch/", cfg.token_contract).as_str())
    {
        return Err(ContractError::WrongRedemptionTokenOrNoFunds {});
    }

    if redemption_coin.amount.is_zero() {
        return Err(ContractError::WrongRedemptionTokenOrNoFunds {});
    }

    // Extract the batch_id
    let parts: Vec<&str> = redemption_coin.denom.split('/').collect();
    if parts.len() != 5 {
        return Err(ContractError::WrongRedemptionTokenOrNoFunds {});
    }

    let batch_id: u64 = parts[4]
        .parse()
        .map_err(|_| ContractError::WrongRedemptionTokenOrNoFunds {})?;

    Ok(batch_id)
}

/// Converts a [`Decimal`] (which stores fixed-point numbers in *atomics*) back
/// into a concrete `Uint128` amount with the desired `decimals` precision.
pub fn dec_to_amount(dec: Decimal, decimals: u32) -> Result<Uint128, ContractError> {
    dec.atomics()
        .checked_div(Uint128::from(10u128.pow(dec.decimal_places() - decimals)))
        .map_err(ContractError::DivideByZeroError)
}
