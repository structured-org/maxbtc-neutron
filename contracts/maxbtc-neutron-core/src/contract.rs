use crate::error::ContractError;
pub(crate) use crate::utils::{dec_to_amount, get_deposit_coin};
use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_ownable::{assert_owner, initialize_owner};
use maxbtc_base::msg::token::ExecuteMsg as TokenExecuteMsg;
use maxbtc_base::msg::{
    core::{
        AllowlistQueryMsg, ConfigResponse, ExchangeRateProviderQueryMsg, ExecuteMsg,
        GetTwaerResponse, InstantiateMsg, MigrateMsg, QueryMsg, SimulateDepositResponse,
        UpdateConfigMsg,
    },
    token::QueryMsg as TokenQueryMsg,
};
use maxbtc_base::state::{
    core::{Config, CONFIG, LAST_DEPOSIT_FLUSH_TIME, TOTAL_DEPOSITED},
    token::Config as TokenConfigResponse,
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
        paused: false,
        token_contract: deps.api.addr_validate(&msg.token_contract)?,
        factory_contract: deps.api.addr_validate(&msg.factory_contract)?,
        deposit_forwarder_contract: deps.api.addr_validate(&msg.deposit_forwarder_contract)?,
        deposit_denom: msg.deposit_denom.clone(),
        deposit_decimals: msg.deposit_decimals,
        deposit_flush_period: msg.deposit_flush_period,
        deposit_cost: msg.deposit_cost,
        deposits_cap: msg.deposits_cap,
        allowlist_contract: deps.api.addr_validate(&msg.allowlist_contract)?,
        exchange_rate_provider_contract: deps
            .api
            .addr_validate(&msg.exchange_rate_provider_contract)?,
        // Store the predicted address in the config
        fee_collector_contract: deps.api.addr_validate(&msg.fee_collector_contract)?,
    };
    CONFIG.save(deps.storage, &cfg)?;

    LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &env.block.time.seconds())?;
    TOTAL_DEPOSITED.save(deps.storage, &Uint128::zero())?;

    // Create the maxBTC denom via token factory
    // let create_maxbtc_denom_msg =
    //     create_tokenfactory_create_denom_msg(&env.clone(), cfg.maxbtc_denom.clone())?;

    // 5. Build the final response with all necessary messages and attributes
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("allowlist_contract", cfg.allowlist_contract.to_string())
        .add_attribute("deposit_denom", cfg.deposit_denom.clone())
        // .add_attribute("maxbtc_denom", cfg.maxbtc_denom.clone())
        .add_attribute(
            "instantiated_fee_collector_address",
            cfg.fee_collector_contract.to_string(),
        )
        .add_attribute("deposit_flush_period", cfg.deposit_flush_period.to_string())
        .add_attribute("deposit_cost", cfg.deposit_cost.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig(updates) => execute_update_config(deps, info, updates),
        ExecuteMsg::Deposit {
            recipient,
            min_receive_amount,
        } => execute_deposit(deps, env, info, recipient, min_receive_amount),
        ExecuteMsg::FlushDeposits {} => execute_flush_deposits(deps, env, info),
        ExecuteMsg::MintFee { amount } => execute_mint_fee(deps, env, info, amount),
        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new().add_attribute("action", "update_ownership"))
        }
    }
}

fn execute_mint_fee(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Coin,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

    if info.sender != cfg.fee_collector_contract {
        return Err(ContractError::Unauthorized {});
    }

    let maxbtc_denom = deps
        .querier
        .query_wasm_smart::<TokenConfigResponse>(&cfg.token_contract, &TokenQueryMsg::Config {})?
        .denom;

    if amount.denom != maxbtc_denom {
        return Err(ContractError::InvalidDepositDenom {
            expected: maxbtc_denom,
            received: amount.denom.to_string(),
        });
    }

    // Mint the maxBTC to the recipient
    let mint_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.token_contract.to_string(),
        msg: to_json_binary(&TokenExecuteMsg::Mint {
            amount: amount.amount,
            recipient: info.sender.to_string(),
        })?,
        funds: vec![],
    });

    // Return the response
    Ok(Response::new()
        .add_message(mint_msg)
        .add_attribute("action", "execute_mint_fee")
        .add_attribute("sender", info.sender)
        .add_attribute("minted_maxbtc", amount.to_string()))
}

/// Owner-only handler that updates the configuration in-place.
fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    updates: UpdateConfigMsg,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;

    // Only the current owner may update the config.
    assert_owner(deps.storage, &info.sender)?;

    // Initialize the response with standard attributes.
    let mut res = Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender.to_string());

    // Apply changes one field at a time and add a corresponding attribute for each update.
    if let Some(paused) = updates.paused {
        cfg.paused = paused;
        res = res.add_attribute("paused_updated", paused.to_string());
    }
    if let Some(addr) = updates.deposit_forwarder_contract {
        let validated_addr = deps.api.addr_validate(&addr)?;
        cfg.deposit_forwarder_contract = validated_addr.clone();
        res = res.add_attribute(
            "deposit_forwarder_contract_updated",
            validated_addr.to_string(),
        );
    }
    if let Some(v) = updates.deposit_flush_period {
        cfg.deposit_flush_period = v;
        res = res.add_attribute("deposit_flush_period_updated", v.to_string());
    }
    if let Some(cap) = updates.deposits_cap {
        cfg.deposits_cap = cap;
        res = res.add_attribute("deposits_cap_updated", cap.unwrap());
    }
    if let Some(deposit_cost) = updates.deposit_cost {
        cfg.deposit_cost = deposit_cost;
        res = res.add_attribute("deposit_cost_updated", deposit_cost.to_string());
    }
    if let Some(allowlist_contract) = updates.allowlist_contract {
        let validated_addr = deps.api.addr_validate(&allowlist_contract)?;
        cfg.allowlist_contract = validated_addr.clone();
        res = res.add_attribute("allowlist_contract_updated", validated_addr.to_string());
    }
    if let Some(exchange_rate_provider_contract) = updates.exchange_rate_provider_contract {
        let validated_addr = deps.api.addr_validate(&exchange_rate_provider_contract)?;
        cfg.exchange_rate_provider_contract = validated_addr.clone();
        res = res.add_attribute(
            "exchange_rate_provider_contract_updated",
            validated_addr.to_string(),
        );
    }
    if let Some(addr) = updates.fee_collector_contract {
        let validated_addr = deps.api.addr_validate(&addr)?;
        cfg.fee_collector_contract = validated_addr.clone();
        res = res.add_attribute("fee_collector_contract_updated", validated_addr.to_string());
    }

    // Save the updated configuration.
    CONFIG.save(deps.storage, &cfg)?;

    Ok(res)
}

pub(crate) fn execute_deposit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    recipient: String,
    min_receive_amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

    // Input funds validation happens here.
    let deposit_coin = get_deposit_coin(cfg.deposit_denom.clone(), info.clone().funds)?;

    // We can't deposit if the total AUM are greater than the cap.
    check_deposit_cap(&deps.as_ref(), &cfg, Some(deposit_coin.amount))?;
    // We can't deposit if the recipient address is not allowlisted.
    check_deposits_allowlist(&deps.as_ref(), &cfg, recipient.clone())?;

    // Calculate the amount of maxBTC to mint.
    let minted_amount = calculate_mint_amount(deps.as_ref(), deposit_coin.amount)?;

    // If a minimum receive amount is specified, ensure we meet that condition
    if let Some(min_amount) = min_receive_amount {
        if minted_amount < min_amount {
            return Err(ContractError::SlippageLimitExceeded {
                requested: min_amount.u128(),
                actual: minted_amount.u128(),
            });
        }
    }

    // Can be equal to zero if rounding kicks in with a very high ER.
    if minted_amount.is_zero() {
        return Err(ContractError::InvalidDepositAmount {});
    }

    // Mint the maxBTC to the recipient
    let mint_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.token_contract.to_string(),
        msg: to_json_binary(&TokenExecuteMsg::Mint {
            amount: minted_amount,
            recipient: recipient.clone(),
        })?,
        funds: vec![],
    });

    TOTAL_DEPOSITED.update(deps.storage, |total| -> Result<Uint128, ContractError> {
        Ok(total + deposit_coin.amount)
    })?;

    // Return the response
    Ok(Response::new()
        .add_message(mint_msg)
        .add_attribute("action", "deposit")
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", recipient)
        .add_attribute("minted_maxbtc", minted_amount.to_string()))
}

/// Flushes the contract's accumulated deposit balance to the deposit forwarder contract.
///
/// This handler can be triggered by any account, but its execution is rate-limited
/// by the `deposit_flush_period` defined in the contract's configuration.
pub(crate) fn execute_flush_deposits(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }
    let mut msgs = vec![];

    let last_time = LAST_DEPOSIT_FLUSH_TIME.load(deps.storage)?;
    let now = env.block.time.seconds();
    if now < last_time + cfg.deposit_flush_period {
        // Not enough time has passed, do nothing
        return Ok(Response::new()
            .add_messages(msgs)
            .add_attribute("action", "flush_deposits")
            .add_attribute("status", "not_enough_time_elapsed"));
    }

    let amount_to_flush = deps
        .querier
        .query_balance(env.contract.address.clone(), cfg.deposit_denom.clone())?
        .amount;

    // Send what's left to the deposit forwarder contract, which will send it to Ethereum over IBC
    // Eureka though the valence library + base account combo
    if !amount_to_flush.is_zero() {
        LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &now)?;
        let msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.deposit_forwarder_contract.to_string(),
            amount: vec![Coin {
                denom: cfg.deposit_denom,
                amount: amount_to_flush,
            }],
        });
        msgs.push(msg);
    }

    let resp = Response::new()
        .add_messages(msgs)
        .add_attribute("action", "flush_deposits")
        .add_attribute("sender", info.sender)
        .add_attribute("flushed", amount_to_flush.to_string());

    Ok(resp)
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::Config {} => {
            let cfg = CONFIG.load(deps.storage)?;
            let resp = ConfigResponse {
                deposit_denom: cfg.deposit_denom,
                deposit_flush_period: cfg.deposit_flush_period,
                deposit_cost: cfg.deposit_cost,
                fee_collector_contract: cfg.fee_collector_contract.to_string(),
            };
            Ok(to_json_binary(&resp)?)
        }
        QueryMsg::ExchangeRate {} => {
            let cfg = CONFIG.load(deps.storage)?;
            let er = get_exchange_rate(&deps, &cfg)
                .map_err(|e| StdError::generic_err(format!("failed to get_exchange_rate: {e}")))?;
            Ok(to_json_binary(&er)?)
        }
        QueryMsg::SimulateDeposit { amount } => {
            // Call the dedicated calculation function and map its error type to StdError
            let minted_amount = calculate_mint_amount(deps, amount)
                .map_err(|e| StdError::generic_err(format!("Calculation failed: {e}")))?;

            let resp = SimulateDepositResponse { minted_amount };
            to_json_binary(&resp)
        }
        QueryMsg::Ownership {} => Ok(to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?),
    }
}

/* -----------------------------------------------------------------------------------------------
/ HELPER FUNCTIONS BELOW
/ -----------------------------------------------------------------------------------------------*/

/// Calculates the amount of maxBTC to be minted for a given deposit amount.
/// This function encapsulates the core logic used in both deposits and simulations.
fn calculate_mint_amount(
    deps: Deps,
    deposit_amount_raw: Uint128,
) -> Result<Uint128, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // Get the current exchange rate
    let er = get_exchange_rate(&deps, &cfg)?;

    // A zero exchange rate is an invalid state and would cause a division by zero error.
    if er.is_zero() {
        // We use InvalidDepositAmount here as a zero exchange rate makes any deposit invalid.
        return Err(ContractError::InvalidDepositAmount {});
    }

    // Convert the raw input amount to a Decimal using the deposit asset's decimals
    let deposit_amount = Decimal::from_atomics(deposit_amount_raw, cfg.deposit_decimals)
        .map_err(|_| ContractError::InvalidDepositAmount {})?;

    // Apply the deposit fee (cost)
    let fee_multiplier = Decimal::one() - cfg.deposit_cost;

    // Calculate the final amount of maxBTC to be minted after fees and exchange rate conversion
    let minted_amount =
        dec_to_amount((deposit_amount * fee_multiplier) / er, cfg.deposit_decimals)?;

    Ok(minted_amount)
}

/// Queries the exchange rate from the exchange rate provider contract.
pub(crate) fn get_exchange_rate(deps: &Deps, cfg: &Config) -> Result<Decimal, ContractError> {
    let res: GetTwaerResponse =
        deps.querier
            .query(&QueryRequest::Wasm(cosmwasm_std::WasmQuery::Smart {
                contract_addr: cfg.exchange_rate_provider_contract.to_string(),
                msg: to_json_binary(&ExchangeRateProviderQueryMsg::GetTwaer {})?,
            }))?;
    Ok(res.twaer)
}

/// Verifies that the current Deposits does **not** exceed the optional *deposit cap*.
fn check_deposit_cap(
    deps: &Deps,
    cfg: &Config,
    deposit: Option<Uint128>,
) -> Result<(), ContractError> {
    if let Some(deposits_cap) = cfg.deposits_cap {
        // Note: real assets under management can be different, but for now we don't care.
        let current_deposits = TOTAL_DEPOSITED.load(deps.storage)?;
        if current_deposits + deposit.unwrap_or_default() > deposits_cap {
            return Err(ContractError::DepositCapExceeded {});
        }
    }

    Ok(())
}

/// Ensures that `recipient` is present in the *allow-list* for deposits by querying
/// the allow-list contract.
fn check_deposits_allowlist(
    deps: &Deps,
    cfg: &Config,
    recipient: String,
) -> Result<(), ContractError> {
    deps.api.addr_validate(&recipient)?;
    let is_allowed: bool =
        deps.querier
            .query(&QueryRequest::Wasm(cosmwasm_std::WasmQuery::Smart {
                contract_addr: cfg.allowlist_contract.to_string(),
                msg: to_json_binary(&AllowlistQueryMsg::IsAddressAllowed { address: recipient })?,
            }))?;
    if !is_allowed {
        Err(ContractError::AddressNotAllowed {})
    } else {
        Ok(())
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}
