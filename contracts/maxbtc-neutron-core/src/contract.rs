use crate::error::ContractError;
use crate::msg::{
    AllowlistQueryMsg, ConfigResponse, ExchangeRateProviderQueryMsg, ExecuteMsg,
    FeeCollectorInstantiateMsg, GetTwaerResponse, InstantiateMsg, MigrateMsg, QueryMsg,
    SimulateDepositResponse, UpdateConfigMsg,
};
use crate::state::{Config, CONFIG, LAST_DEPOSIT_FLUSH_TIME, TOTAL_DEPOSITED};
pub(crate) use crate::utils::{dec_to_amount, get_deposit_coin};
use cosmwasm_std::{
    entry_point, instantiate2_address, to_json_binary, BankMsg, Coin, CosmosMsg, Decimal, Deps,
    DepsMut, Env, MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_ownable::{assert_owner, initialize_owner};
use neutron_std::types::cosmos::base::v1beta1::Coin as BaseCoin;
use neutron_std::types::osmosis::tokenfactory::v1beta1::{MsgCreateDenom, MsgMint};

const CONTRACT_NAME: &str = "maxbtc-neutron-minting";
const CONTRACT_VERSION: &str = "0.1.0";

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Get the checksum for the fee collector contract's code ID
    let fee_collector_code_info = deps
        .querier
        .query_wasm_code_info(msg.fee_collector_params.code_id)?;
    let fee_collector_checksum = fee_collector_code_info.checksum;

    // Predict the fee collector contract address using Instantiate2
    let canonical_creator = deps.api.addr_canonicalize(env.contract.address.as_str())?;
    let fee_collector_address = instantiate2_address(
        fee_collector_checksum.as_slice(),
        &canonical_creator, // The creator is this core contract
        &msg.fee_collector_params.salt,
    )
    .map_err(ContractError::Instantiate2Error)?;

    initialize_owner(deps.storage, deps.api, Some(&msg.owner))?;

    // Build the Config, now with the predictable fee collector address
    let cfg = Config {
        paused: false,
        deposit_forwarder_contract: deps.api.addr_validate(&msg.deposit_forwarder_contract)?,
        deposit_denom: msg.deposit_denom.clone(),
        deposit_decimals: msg.deposit_decimals,
        maxbtc_denom: msg.maxbtc_denom.clone(),
        deposit_flush_period: msg.deposit_flush_period,
        deposit_cost: msg.deposit_cost,
        deposits_cap: msg.deposits_cap,
        allowlist_contract: deps.api.addr_validate(&msg.allowlist_contract)?,
        exchange_rate_provider_contract: deps
            .api
            .addr_validate(&msg.exchange_rate_provider_contract)?,
        // Store the predicted address in the config
        fee_collector_contract: deps.api.addr_humanize(&fee_collector_address)?,
    };
    CONFIG.save(deps.storage, &cfg)?;

    LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &env.block.time.seconds())?;
    TOTAL_DEPOSITED.save(deps.storage, &Uint128::zero())?;

    // Create the instantiate message for the fee collector contract
    let instantiate_fee_collector_msg = WasmMsg::Instantiate2 {
        admin: Some(msg.owner.to_string()), // The core contract owner is admin
        code_id: msg.fee_collector_params.code_id,
        label: "maxBTC Fee Collector Contract".to_string(),
        msg: to_json_binary(&FeeCollectorInstantiateMsg {
            owner: msg.owner.to_string(), // Same owner as the core contract
            core_contract: env.contract.address.to_string(), // This contract's address
            fee_apy_reduction_percentage: msg.fee_collector_params.fee_apy_reduction_percentage,
            collection_period_seconds: msg.fee_collector_params.collection_period_seconds,
            fee_denom: cfg.get_maxbtc_denom(env.contract.address.to_string()),
            maxbtc_decimals: cfg.deposit_decimals,
        })?,
        funds: vec![],
        salt: msg.fee_collector_params.salt.clone(),
    };

    // Create the maxBTC denom via token factory
    let create_maxbtc_denom_msg =
        create_tokenfactory_create_denom_msg(&env.clone(), cfg.maxbtc_denom.clone())?;

    // 5. Build the final response with all necessary messages and attributes
    Ok(Response::new()
        .add_message(create_maxbtc_denom_msg)
        .add_message(instantiate_fee_collector_msg) // Add the message to instantiate the fee collector
        .add_attribute("action", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("allowlist_contract", cfg.allowlist_contract.to_string())
        .add_attribute("deposit_denom", cfg.deposit_denom.clone())
        .add_attribute("maxbtc_denom", cfg.maxbtc_denom.clone())
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
    env: Env,
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

    if amount.denom != cfg.get_maxbtc_denom(env.contract.address.to_string()) {
        return Err(ContractError::InvalidDepositDenom {
            expected: cfg.get_maxbtc_denom(env.contract.address.to_string()),
            received: amount.denom.to_string(),
        });
    }

    // Mint the maxBTC to the recipient
    let mint_msg =
        create_tokenfactory_mint_msg(&env.clone(), info.sender.to_string(), amount.clone())?;

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
    env: Env,
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
    let mint_msg = create_tokenfactory_mint_msg(
        &env.clone(),
        recipient.clone(),
        Coin {
            amount: minted_amount,
            denom: cfg.get_maxbtc_denom(env.contract.address.to_string()),
        },
    )?;

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
                maxbtc_denom: cfg.maxbtc_denom,
                deposit_flush_period: cfg.deposit_flush_period,
                deposit_cost: cfg.deposit_cost,
                fee_collector_contract: cfg.fee_collector_contract.to_string(),
            };
            Ok(to_json_binary(&resp)?)
        }
        QueryMsg::ExchangeRate {} => {
            let cfg = CONFIG.load(deps.storage)?;
            let er = get_exchange_rate(&deps, &cfg).map_err(|e| {
                StdError::generic_err(format!("failed to get_exchange_rate: {}", e))
            })?;
            Ok(to_json_binary(&er)?)
        }
        QueryMsg::SimulateDeposit { amount } => {
            // Call the dedicated calculation function and map its error type to StdError
            let minted_amount = calculate_mint_amount(deps, amount)
                .map_err(|e| StdError::generic_err(format!("Calculation failed: {}", e)))?;

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

/// Creates a message to create a tokenfactory denom.
fn create_tokenfactory_create_denom_msg(env: &Env, denom: String) -> StdResult<CosmosMsg> {
    Ok(Into::<CosmosMsg>::into(MsgCreateDenom {
        sender: env.contract.address.to_string(),
        subdenom: denom,
    }))
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
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    if let Some(mint) = msg.mint {
        let cfg = CONFIG.load(deps.storage)?;

        let mint_msg = create_tokenfactory_mint_msg(
            &env.clone(),
            mint.recipient.clone(),
            Coin {
                amount: mint.amount,
                denom: cfg.get_maxbtc_denom(env.contract.address.to_string()),
            },
        )?;

        return Ok(Response::new()
            .add_message(mint_msg)
            .add_attribute("action", "migrate_mint")
            .add_attribute("recipient", mint.recipient)
            .add_attribute("minted_maxbtc", mint.amount.to_string()));
    }
    Ok(Response::default())
}
