use crate::error::ContractError;
use crate::msg::{
    BatchResponse, CollectorExecuteMsg, ConfigResponse, ExecuteMsg, FeeCollectorInstantiateMsg,
    InstantiateMsg, LiquidationBufferContractQueryMsg, LiquidationBufferExecuteMsg, OracleQueryMsg,
    QueryMsg, UpdateConfigMsg,
};
use crate::state::{
    Batch, CachedAUM, CachedER, Config, ContractState, ACTIVE_BATCH, ACTIVE_BATCH_START_TIME,
    BATCH_ID_COUNTER, CACHED_ER, CONFIG, FINALIZED_BATCHES, FSM, LAST_DEPOSIT_FLUSH_TIME,
    WITHDRAWING_BATCH,
};
pub(crate) use crate::utils::{dec_to_amount, get_deposit_coin, Aum};
use cosmwasm_std::{
    entry_point, instantiate2_address, to_json_binary, BankMsg, BankQuery, Coin, CosmosMsg,
    Decimal, Deps, DepsMut, Env, MessageInfo, QueryRequest, Response, StdError, StdResult,
    SupplyResponse, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use neutron_std::types::cosmos::base::v1beta1::Coin as BaseCoin;
use neutron_std::types::osmosis::tokenfactory::v1beta1::{MsgBurn, MsgCreateDenom, MsgMint};

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

    // Get the checksum for the fee minter contract's code ID
    let fee_minter_code_info = deps
        .querier
        .query_wasm_code_info(msg.fee_collector_params.code_id)?;
    let fee_minter_checksum = fee_minter_code_info.checksum;

    // Predict the fee minter contract address using Instantiate2
    let canonical_creator = deps.api.addr_canonicalize(env.contract.address.as_str())?;
    let fee_minter_address = instantiate2_address(
        fee_minter_checksum.as_slice(),
        &canonical_creator, // The creator is this core contract
        &msg.fee_collector_params.salt,
    )
    .map_err(|e| ContractError::Instantiate2Error(e))?;

    // Build the Config, now with the predictable fee minter address
    let cfg = Config {
        paused: false,
        owner: deps.api.addr_validate(&msg.owner)?,
        aum_oracle_contract: deps.api.addr_validate(&msg.aum_contract)?,
        liquidation_buffer_contract: deps.api.addr_validate(&msg.liquidation_buffer_contract)?,
        deposit_pump_contract: deps.api.addr_validate(&msg.deposit_pump_contract)?,
        collector_contract: deps.api.addr_validate(&msg.collector_contract)?,
        treasury_address: deps.api.addr_validate(&msg.treasury_address)?,
        deposit_denom: msg.deposit_denom.clone(),
        deposit_decimals: msg.deposit_decimals,
        maxbtc_denom: msg.maxbtc_denom.clone(),
        deposit_flush_period: msg.deposit_flush_period,
        batch_active_duration: msg.batch_active_duration,
        batch_withdrawing_duration: msg.batch_withdrawing_duration,
        collected_tolerance: msg.accepted_withdrawable_percentage,
        liquidation_buffer_share: msg.liquidation_buffer_share,
        deposit_cost: msg.deposit_cost,
        deposit_buffer_tolerance: msg.cached_aum_tolerance,
        cached_er_ttl: msg.cached_er_ttl,
        deposits_cap: msg.deposits_cap,
        deposits_allowlist: msg
            .deposits_allowlist
            .map(|v| {
                v.into_iter()
                    .map(|s| deps.api.addr_validate(&s))
                    .collect::<StdResult<_>>()
            })
            .transpose()?,
        // Store the predicted address in the config
        fee_minter_contract: deps.api.addr_humanize(&fee_minter_address)?,
    };
    CONFIG.save(deps.storage, &cfg)?;

    // (The rest of the state initialization remains the same)
    BATCH_ID_COUNTER.save(deps.storage, &0u64)?;
    WITHDRAWING_BATCH.save(deps.storage, &None)?;
    LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &env.block.time.seconds())?;
    ACTIVE_BATCH_START_TIME.save(deps.storage, &env.block.time.seconds())?;
    CACHED_ER.save(deps.storage, &None)?;

    let new_batch_id = 1u64;
    let new_batch = Batch {
        batch_id: new_batch_id,
        btc_requested: Uint128::zero(),
        maxbtc_burned: Default::default(),
        collected_amount: Uint128::zero(),
        paid_amount: Uint128::zero(),
        collector_historical_balance: Uint128::zero(),
    };
    ACTIVE_BATCH.save(deps.storage, &Some(new_batch))?;
    BATCH_ID_COUNTER.save(deps.storage, &new_batch_id)?;

    // Create the instantiate message for the fee collector contract
    let instantiate_fee_collector_msg = WasmMsg::Instantiate2 {
        admin: Some(cfg.owner.to_string()), // The core contract owner is admin
        code_id: msg.fee_collector_params.code_id,
        label: "maxBTC Fee Collector Contract".to_string(),
        msg: to_json_binary(&FeeCollectorInstantiateMsg {
            owner: msg.owner,                                // Same owner as the core contract
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

    // Initialise the FSM
    FSM.set_initial_state(deps.storage, ContractState::Idle)?;

    // 5. Build the final response with all necessary messages and attributes
    Ok(Response::new()
        .add_message(create_maxbtc_denom_msg)
        .add_message(instantiate_fee_collector_msg) // Add the message to instantiate the fee collector
        .add_attribute("action", "instantiate")
        .add_attribute("owner", cfg.owner.to_string())
        .add_attribute("aum_contract", cfg.aum_oracle_contract.to_string())
        .add_attribute(
            "liquidation_buffer_contract",
            cfg.liquidation_buffer_contract.to_string(),
        )
        .add_attribute("collector_contract", cfg.collector_contract.to_string())
        .add_attribute("treasury_address", cfg.treasury_address.to_string())
        .add_attribute("deposit_denom", cfg.deposit_denom.clone())
        .add_attribute("maxbtc_denom", cfg.maxbtc_denom.clone())
        .add_attribute(
            "instantiated_fee_minter_address",
            fee_minter_address.to_string(),
        )
        .add_attribute("deposit_flush_period", cfg.deposit_flush_period.to_string())
        .add_attribute(
            "batch_active_duration",
            cfg.batch_active_duration.to_string(),
        )
        .add_attribute(
            "batch_withdrawing_duration",
            cfg.batch_withdrawing_duration.to_string(),
        )
        .add_attribute(
            "accepted_withdrawable_percentage",
            cfg.collected_tolerance.to_string(),
        )
        .add_attribute(
            "liquidation_buffer_share",
            cfg.liquidation_buffer_share.to_string(),
        )
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
        ExecuteMsg::Deposit { recipient } => execute_deposit(deps, env, info, recipient),
        ExecuteMsg::FlushDeposits {} => execute_flush_deposits(deps, env, info),
        ExecuteMsg::Withdraw {} => execute_withdraw(deps, env, info),
        ExecuteMsg::ProcessActiveBatch {} => execute_process_active_batch(deps, env, info),
        ExecuteMsg::Claim { recipient } => execute_claim(deps, env, info, recipient),
        ExecuteMsg::ProcessCache {} => {
            let cfg = CONFIG.load(deps.storage)?;
            let msgs = _process_cache(deps, env, &cfg)?;
            Ok(Response::new()
                .add_messages(msgs)
                .add_attribute("action", "process_cache"))
        }
        ExecuteMsg::MintFee { amount } => execute_mint_fee(deps, env, info, amount),
    }
}

fn execute_mint_fee(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Coin,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.fee_minter_contract {
        return Err(ContractError::Unauthorized {});
    }

    if amount.denom != cfg.maxbtc_denom {
        return Err(ContractError::InvalidDepositDenom {
            expected: cfg.maxbtc_denom.to_string(),
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
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Apply changes one field at a time.
    if let Some(paused) = updates.paused {
        cfg.paused = paused;
    }
    if let Some(owner) = updates.owner {
        cfg.owner = deps.api.addr_validate(&owner)?;
    }
    if let Some(addr) = updates.aum_contract {
        cfg.aum_oracle_contract = deps.api.addr_validate(&addr)?;
    }
    if let Some(addr) = updates.liquidation_contract {
        cfg.liquidation_buffer_contract = deps.api.addr_validate(&addr)?;
    }
    if let Some(addr) = updates.deposit_pump_contract {
        cfg.deposit_pump_contract = deps.api.addr_validate(&addr)?;
    }
    if let Some(addr) = updates.collector_contract {
        cfg.collector_contract = deps.api.addr_validate(&addr)?;
    }
    if let Some(addr) = updates.treasury_address {
        cfg.treasury_address = deps.api.addr_validate(&addr)?;
    }
    if let Some(v) = updates.deposit_flush_period {
        cfg.deposit_flush_period = v;
    }
    if let Some(v) = updates.batch_active_duration {
        cfg.batch_active_duration = v;
    }
    if let Some(v) = updates.batch_withdrawing_duration {
        cfg.batch_withdrawing_duration = v;
    }
    if let Some(v) = updates.accepted_withdrawable_percentage {
        cfg.collected_tolerance = v;
    }
    if let Some(v) = updates.liquidation_buffer_share {
        cfg.liquidation_buffer_share = v;
    }
    if let Some(v) = updates.deposit_cost {
        cfg.deposit_cost = v;
    }
    if let Some(v) = updates.cached_aum_tolerance {
        cfg.deposit_buffer_tolerance = v;
    }
    if let Some(v) = updates.cached_er_ttl {
        cfg.cached_er_ttl = v;
    }
    if let Some(cap) = updates.deposits_cap {
        cfg.deposits_cap = cap;
    }
    if let Some(maybe_list) = updates.deposits_allowlist {
        cfg.deposits_allowlist = maybe_list
            .map(|v| {
                v.into_iter()
                    .map(|s| deps.api.addr_validate(&s))
                    .collect::<StdResult<_>>()
            })
            .transpose()?;
    }
    if let Some(addr) = updates.fee_collector_contract {
        cfg.fee_minter_contract = deps.api.addr_validate(&addr)?;
    }

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
}

pub(crate) fn execute_deposit(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

    // We can't deposit if the total AUM are greater than the cap.
    check_deposit_cap(&deps.as_ref(), env.clone(), &cfg)?;
    // We can't deposit if the recipient address is not allowlisted.
    check_deposits_allowlist(&deps.as_ref(), &cfg, recipient.clone())?;

    let mut msgs = _process_cache(deps.branch(), env.clone(), &cfg)?;

    // Input funds validation happens here.
    let deposit_coin = get_deposit_coin(cfg.deposit_denom.clone(), info.funds)?;

    // Get the exchange rate
    let er = get_exchange_rate(
        &deps.as_ref(),
        env.clone(),
        &cfg.clone(),
        Some(deposit_coin.amount),
    )?;

    // Adjust for deposit fee
    let deposit_amount = Decimal::from_atomics(deposit_coin.amount, cfg.deposit_decimals)
        .map_err(|_| ContractError::InvalidDepositAmount {})?;

    // Apply the deposit fee and divide by exchange rate
    let fee_multiplier = Decimal::one() - cfg.deposit_cost;

    // CosmWasm's Decimal always uses 18 digits internally, so scale down
    // to match cfg.deposit_decimals (e.g., 6) before minting.
    let minted_amount =
        dec_to_amount((deposit_amount * fee_multiplier) / er, cfg.deposit_decimals)?;

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
    msgs.push(mint_msg);

    // Return the response
    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "deposit")
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", recipient)
        .add_attribute("minted_maxbtc", minted_amount.to_string()))
}

/// Permissionless deposit flush
/// - checks time has passed at least deposit_flush_period
/// - if liquidation buffer contract holds less than liquidation_buffer_share of AUM, send enough
/// - if liquidation buffer contract holds more, request some back (it will be processed next time)
/// - then IBC Eureka transfer everything else to the custody
pub(crate) fn execute_flush_deposits(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }
    let mut msgs = _process_cache(deps.branch(), env.clone(), &cfg)?;

    let last_time = LAST_DEPOSIT_FLUSH_TIME.load(deps.storage)?;
    let now = env.block.time.seconds();
    if now < last_time + cfg.deposit_flush_period {
        // Not enough time has passed, do nothing
        return Ok(Response::new()
            .add_messages(msgs)
            .add_attribute("action", "flush_deposits")
            .add_attribute("status", "not_enough_time_elapsed"));
    }

    let aum = get_aum(&deps.as_ref(), env.clone(), &cfg)?;
    let mut amount_to_flush = aum.deposit_buffer;

    if amount_to_flush.is_zero() {
        // There have been no deposits, set last flush time to now and wait for another
        // cfg.deposit_flush_period
        LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &now)?;

        return Ok(Response::new()
            .add_attribute("action", "flush_deposits")
            .add_attribute("status", "zero_outstanding_deposits"));
    }

    FSM.go_to(deps.storage, ContractState::Flushing)?;

    // The "liquidation buffer" we want is liquidation_buffer_share * total aum
    let required_buffer = dec_to_amount(
        Decimal::from_atomics(aum.total(), cfg.deposit_decimals)? * cfg.liquidation_buffer_share,
        cfg.deposit_decimals,
    )?;

    // If liquidation buffer contract < required => send the difference
    if aum.liquidation_buffer_contract < required_buffer {
        let mut to_send_to_liquidation_buffer_contract =
            required_buffer - aum.liquidation_buffer_contract;
        // We are allowed to exhaust the deposit buffer completely.
        if to_send_to_liquidation_buffer_contract > amount_to_flush {
            to_send_to_liquidation_buffer_contract = amount_to_flush
        }
        // We do a bank send of the deposit_denom from this contract to the liquidation buffer contract
        let msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.liquidation_buffer_contract.to_string(),
            amount: vec![Coin {
                denom: cfg.deposit_denom.clone(),
                amount: to_send_to_liquidation_buffer_contract,
            }],
        });
        msgs.push(msg);
        amount_to_flush -= to_send_to_liquidation_buffer_contract;
    } else {
        // If liquidation buffer contract > required => call liquidation buffer contract's method to
        // send back the difference
        let to_recv = Coin {
            amount: aum.liquidation_buffer_contract - required_buffer,
            denom: cfg.deposit_denom.clone(),
        };
        // The returned funds will be processed next time.
        if to_recv.amount > Uint128::zero() {
            let msg = create_liquidation_buffer_clawback_msg(
                cfg.liquidation_buffer_contract.to_string(),
                to_recv.clone(),
            )?;
            msgs.push(msg);
            amount_to_flush += to_recv.amount;
        }
    }

    // Cache the exchange rate, oracle_aum and deposit_buffer value, because we need it
    // in process_cache() to check whether the deposit reached Binance / Solana.
    // Note: we store `amount_to_flush` to `deposit_buffer` because that's how much will
    // reach the remote chain. Saving `aum.deposit_buffer` would lead to _process_cache_flushing()
    // expect more than was actually flushed.
    let er = get_exchange_rate(&deps.as_ref(), env.clone(), &cfg, None)?;
    CACHED_ER.save(
        deps.storage,
        &Some(CachedER {
            er,
            timeout: now + cfg.cached_er_ttl,
            aum: Some(CachedAUM {
                oracle_aum: aum.oracle_aum,
                deposit_buffer: amount_to_flush,
            }),
        }),
    )?;

    // Send what's left to the deposit pump contract, which will send it to Ethereum over IBC
    // Eureka
    if !amount_to_flush.is_zero() {
        let msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.deposit_pump_contract.to_string(),
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
        .add_attribute("liquidation_buffer", required_buffer.to_string())
        .add_attribute("flushed", amount_to_flush.to_string());

    Ok(resp)
}

/// User requests to withdraw BTC and burn their maxBTC
pub(crate) fn execute_withdraw(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

    let mut msgs = _process_cache(deps.branch(), env.clone(), &cfg)?;

    // Input funds validation happens here
    let burned_amount = get_deposit_coin(
        cfg.get_maxbtc_denom(env.contract.address.to_string()),
        info.funds,
    )?;

    // Burn the maxBTC from user
    let burn_msg = create_tokenfactory_burn_msg(
        env.clone(),
        burned_amount.clone(),
        env.contract.address.to_string(),
    )?;
    msgs.push(burn_msg);

    // Update the maxbtc_burned amount in the active batch
    let mut active_batch = ACTIVE_BATCH
        .load(deps.storage)?
        .ok_or(ContractError::BatchStateError {})?;
    active_batch.maxbtc_burned += burned_amount.amount;
    ACTIVE_BATCH.save(deps.storage, &Some(active_batch.clone()))?;

    // Mint the redemption tokens (1:1 maxBTC burned)
    let minted_redemption = burned_amount.amount;

    let redemption_denom_base = format!("redemption/batch/{}", active_batch.batch_id);
    let redemption_denom_full =
        cfg.get_redemption_denom(env.contract.address.to_string(), active_batch.batch_id);
    let total_redemption_supply = deps.querier.query_supply(redemption_denom_full.clone())?;
    if total_redemption_supply.amount.is_zero() {
        msgs.push(create_tokenfactory_create_denom_msg(
            &env,
            redemption_denom_base.clone(),
        )?)
    }

    // Construct a message to mint redemption tokens
    let mint_redemption_msg = create_tokenfactory_mint_msg(
        &env,
        info.sender.to_string(),
        Coin {
            amount: minted_redemption,
            denom: redemption_denom_full.clone(),
        },
    )?;

    let resp = Response::new()
        .add_messages(msgs)
        .add_message(mint_redemption_msg)
        .add_attribute("action", "withdraw")
        .add_attribute("sender", info.sender)
        .add_attribute("batch_id", active_batch.batch_id.to_string())
        .add_attribute("withdraw_amount", burned_amount.amount.to_string());

    Ok(resp)
}

/// Permissionless call to move batch ACTIVE → WITHDRAWING if time is up
pub(crate) fn execute_process_active_batch(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }
    let msgs = _process_cache(deps.branch(), env.clone(), &cfg)?;

    let now = env.block.time.seconds();
    let active_start_time = ACTIVE_BATCH_START_TIME.load(deps.storage)?;

    if now < active_start_time + cfg.batch_active_duration {
        return Ok(Response::new()
            .add_messages(msgs)
            .add_attribute("action", "process_active_batch")
            .add_attribute("status", "not_enough_time_elapsed"));
    }

    // If the active batch has no redemption tokens minted,
    // it means no one wants to withdraw, so just reset the timer and do nothing
    let active_batch = ACTIVE_BATCH
        .load(deps.storage)?
        .ok_or(ContractError::BatchStateError {})?;
    let redemption_token_supply = query_token_supply(
        &deps.as_ref(),
        cfg.get_redemption_denom(env.contract.address.to_string(), active_batch.batch_id),
    )?;
    if redemption_token_supply.is_zero() {
        // reset the start_time to now, so the next cycle begins
        ACTIVE_BATCH_START_TIME.save(deps.storage, &now)?;
        return Ok(Response::new()
            .add_messages(msgs)
            .add_attribute("action", "process_active_batch")
            .add_attribute("status", "no_withdraw_requests_found"));
    }

    FSM.go_to(deps.storage, ContractState::Withdrawing)?;

    let mut withdrawing_batch = Batch {
        batch_id: active_batch.batch_id,
        btc_requested: Uint128::zero(),
        maxbtc_burned: active_batch.maxbtc_burned,
        collected_amount: Uint128::zero(),
        paid_amount: Uint128::zero(),
        collector_historical_balance: Uint128::zero(),
    };
    // Record collector_historical_balance
    let collector_balance = deps
        .querier
        .query_balance(&cfg.collector_contract, &cfg.deposit_denom)?;
    withdrawing_batch.collector_historical_balance = collector_balance.amount;

    // Get the exchange rate
    let er = get_exchange_rate(&deps.as_ref(), env.clone(), &cfg.clone(), None)?;

    // The total BTC requested = total_redemption_supply * er.
    // Note: all of our tokens have the same number of decimals as the
    // deposit denom.
    withdrawing_batch.btc_requested = dec_to_amount(
        er * Decimal::from_atomics(redemption_token_supply, cfg.deposit_decimals)?,
        cfg.deposit_decimals,
    )?;

    // Save it in WITHDRAWING_BATCH
    WITHDRAWING_BATCH.save(deps.storage, &Some(withdrawing_batch.clone()))?;

    // Create a new ACTIVE batch
    let mut batch_id_counter = BATCH_ID_COUNTER.load(deps.storage)?;
    batch_id_counter += 1;
    let new_active_batch = Batch {
        batch_id: batch_id_counter,
        btc_requested: Uint128::zero(),
        maxbtc_burned: Uint128::zero(),
        collected_amount: Uint128::zero(),
        paid_amount: Uint128::zero(),
        collector_historical_balance: Uint128::zero(),
    };
    ACTIVE_BATCH.save(deps.storage, &Some(new_active_batch))?;
    BATCH_ID_COUNTER.save(deps.storage, &batch_id_counter)?;
    ACTIVE_BATCH_START_TIME.save(deps.storage, &now)?;

    // Cache the exchange rate; aum is not cached because it's irrelevant for the
    // WITHDRAWING -> IDLE transition
    CACHED_ER.save(
        deps.storage,
        &Some(CachedER {
            aum: None,
            er,
            timeout: now + cfg.cached_er_ttl,
        }),
    )?;

    let resp = Response::new()
        .add_messages(msgs)
        .add_attribute("action", "process_active_batch")
        .add_attribute("sender", info.sender)
        .add_attribute(
            "new_withdrawing_batch_id",
            active_batch.batch_id.to_string(),
        )
        .add_attribute("btc_requested", active_batch.btc_requested.to_string());
    Ok(resp)
}

/// User claims their BTC, providing redemption tokens as input
pub(crate) fn execute_claim(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }
    let mut msgs = _process_cache(deps.branch(), env.clone(), &cfg)?;

    if info.funds.len() != 1 {
        return Err(ContractError::WrongRedemptionTokenOrNoFunds {});
    }
    let redemption_coin = &info.funds[0];
    let batch_id = get_batch_id_from_redemption_coin(env.clone(), redemption_coin.clone())?;

    // Check the batch is in FINALIZED state
    let finalized_batch = FINALIZED_BATCHES.may_load(deps.storage, batch_id)?;
    let mut batch = match finalized_batch {
        Some(b) => b,
        None => return Err(ContractError::BatchNotFinalized {}),
    };

    // The user’s portion = collected_amount * (user_redemption_tokens / total_redemption_tokens)
    let redemption_token_supply =
        query_token_supply(&deps.as_ref(), redemption_coin.denom.clone())?;
    if redemption_token_supply.is_zero() {
        return Err(ContractError::RedemptionSupplyMismatch {});
    }
    let user_amount_dec = Decimal::from_atomics(redemption_coin.amount, cfg.deposit_decimals)?;
    let redemption_token_supply_dec =
        Decimal::from_atomics(redemption_token_supply, cfg.deposit_decimals)?;
    let fraction = user_amount_dec / redemption_token_supply_dec;

    let available_dec = Decimal::from_atomics(
        batch.collected_amount - batch.paid_amount,
        cfg.deposit_decimals,
    )?;
    let user_btc = dec_to_amount(available_dec * fraction, cfg.deposit_decimals)?;

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
    let burn_msg = create_tokenfactory_burn_msg(
        env.clone(),
        redemption_coin.clone(),
        env.contract.address.to_string(),
    )?;
    msgs.push(burn_msg);

    // Update the paid out amount in the batch
    batch.paid_amount += user_btc;
    FINALIZED_BATCHES.save(deps.storage, batch.batch_id, &batch)?;

    let resp = Response::new()
        .add_messages(msgs)
        .add_attribute("action", "claim")
        .add_attribute("batch_id", batch_id.to_string())
        .add_attribute("user_claim_btc", user_btc.to_string());

    Ok(resp)
}

/// Orchestrates completion of long-running, multi-block operations by
/// “draining” the cached exchange-rate (ER) object and driving the contract’s
/// finite-state machine (FSM) back to `Idle`.
///
/// The contract performs some actions (deposit flush, withdrawal batch) that
/// cannot be completed atomically inside a single transaction.  Each of those
/// actions:
/// 1. Stores a snapshot of the **exchange-rate cache** (`CACHED_ER`) together
///    with a **timeout**;
/// 2. Moves the FSM to an *intermediate* state (`Flushing` or `Withdrawing`).
///
/// `_process_cache` must be called at the start of every externally-facing
/// entry-point that can mutate balances / state.
pub(crate) fn _process_cache(
    deps: DepsMut,
    env: Env,
    cfg: &Config,
) -> Result<Vec<CosmosMsg>, ContractError> {
    // If there is no cache, there is nothing to do.
    let cached_er = match CACHED_ER.load(deps.storage)? {
        Some(cached_er) => cached_er,
        None => return Ok(vec![]),
    };

    // The cache is stale, we can not perform any operations
    if env.block.time.seconds() > cached_er.timeout {
        return Err(ContractError::ProtocolInEmergency {});
    }

    let fsm_state = FSM.get_current_state(deps.storage)?;
    match fsm_state {
        ContractState::Idle => {
            // If the protocol is in idle state, we can not have a cached ER
            Err(ContractError::ProtocolInEmergency {})
        }
        ContractState::Flushing => _process_cache_flushing(deps, cfg, cached_er),
        ContractState::Withdrawing => _process_cache_withdrawing(deps, cfg),
    }
}

fn _process_cache_flushing(
    deps: DepsMut,
    cfg: &Config,
    cached_er: CachedER,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let current_oracle_aum: Uint128 = deps.querier.query_wasm_smart(
        cfg.aum_oracle_contract.to_string(),
        &OracleQueryMsg::GetAUM {},
    )?;

    // If we are flushing, cached_aum must be present
    let cached_aum = cached_er.aum.ok_or(ContractError::ProtocolInEmergency {})?;
    let historical_oracle_aum = cached_aum.oracle_aum;

    // This is, strictly speaking, not an emergency (can occur naturally due to price fluctuations,
    // especially if this code is executed right after the deposit buffer was flushed).
    // We simply return Ok() and stay with the cached ER that we have.
    if current_oracle_aum < historical_oracle_aum {
        return Ok(vec![]);
    }

    // If the cache is not stale, we need to check whether the amount that reached Binance / Solana
    // is close enough to the previously flushed deposit buffer; if that is the case, we can make
    // the FLUSHING -> IDLE transition and discard the cache.
    let successfully_flushed = current_oracle_aum - historical_oracle_aum;
    let cached_deposit_buffer_dec =
        Decimal::from_atomics(cached_aum.deposit_buffer, cfg.deposit_decimals)?;
    let accepted_diff = dec_to_amount(
        cached_deposit_buffer_dec * cfg.deposit_buffer_tolerance,
        cfg.deposit_decimals,
    )?;
    if successfully_flushed > cached_aum.deposit_buffer
        || (cached_aum.deposit_buffer - successfully_flushed) < accepted_diff
    {
        FSM.go_to(deps.storage, ContractState::Idle)?;
        CACHED_ER.save(deps.storage, &None)?;
    }

    // The previously flushed deposit didn't come through yet, but the cache is
    // not stale either; no issue here, we keep using the cached AUM.
    Ok(vec![])
}

fn _process_cache_withdrawing(
    deps: DepsMut,
    cfg: &Config,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let mut msgs: Vec<CosmosMsg> = vec![];

    // If we are in a withdrawing state, there has to be a withdrawing batch
    let withdrawing_batch = WITHDRAWING_BATCH
        .load(deps.storage)?
        .ok_or(ContractError::ProtocolInEmergency {})?;

    // Calculate the collected amount
    let current_collector_balance = deps
        .querier
        .query_balance(&cfg.collector_contract, &cfg.deposit_denom)?;
    let historical_collector_balance = withdrawing_batch.collector_historical_balance;

    // Historical balance can not be greater that current balance
    if current_collector_balance.amount < historical_collector_balance {
        return Err(ContractError::ProtocolInEmergency {});
    }

    let mut collected = current_collector_balance.amount - historical_collector_balance;

    let requested_dec =
        Decimal::from_atomics(withdrawing_batch.btc_requested, cfg.deposit_decimals)?;
    let accepted_diff = dec_to_amount(
        requested_dec * cfg.collected_tolerance,
        cfg.deposit_decimals,
    )?;

    // If we collected more than requested, or within the `accepted_diff` less than requested,
    // finalize the batch and transition WITHDRAWING -> IDLE.
    if collected > withdrawing_batch.btc_requested
        || withdrawing_batch.btc_requested - collected < accepted_diff
    {
        if collected > withdrawing_batch.btc_requested {
            let extra = collected - withdrawing_batch.btc_requested;
            collected -= extra;
            // Send `extra` to treasury
            let send_msg: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
                to_address: cfg.treasury_address.to_string(),
                amount: vec![Coin {
                    denom: cfg.deposit_denom.clone(),
                    amount: extra,
                }],
            });
            msgs.push(send_msg);
        }

        // Claim the collected amount
        let receive_msg = create_collector_claim_msg(
            cfg.collector_contract.to_string(),
            Coin {
                denom: cfg.deposit_denom.clone(),
                amount: collected,
            },
        )?;
        msgs.push(receive_msg);

        let finalized_batch = Batch {
            batch_id: withdrawing_batch.batch_id,
            btc_requested: withdrawing_batch.btc_requested,
            maxbtc_burned: withdrawing_batch.maxbtc_burned,
            collected_amount: collected,
            paid_amount: Uint128::zero(),
            collector_historical_balance: withdrawing_batch.collector_historical_balance,
        };

        FSM.go_to(deps.storage, ContractState::Idle)?;
        CACHED_ER.save(deps.storage, &None)?;
        WITHDRAWING_BATCH.save(deps.storage, &None)?;
        FINALIZED_BATCHES.save(deps.storage, finalized_batch.batch_id, &finalized_batch)?;
    }

    Ok(msgs)
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::Config {} => {
            let cfg = CONFIG.load(deps.storage)?;
            let resp = ConfigResponse {
                owner: cfg.owner.to_string(),
                aum_contract: cfg.aum_oracle_contract.to_string(),
                liquidation_contract: cfg.liquidation_buffer_contract.to_string(),
                treasury_address: cfg.treasury_address.to_string(),
                deposit_denom: cfg.deposit_denom,
                maxbtc_denom: cfg.maxbtc_denom,
                deposit_flush_period: cfg.deposit_flush_period,
                batch_active_duration: cfg.batch_active_duration,
                batch_withdrawing_duration: cfg.batch_withdrawing_duration,
                accepted_withdrawable_percentage: cfg.collected_tolerance,
                liquidation_buffer_share: cfg.liquidation_buffer_share,
                deposit_cost: cfg.deposit_cost,
            };
            Ok(to_json_binary(&resp)?)
        }
        QueryMsg::ActiveBatch {} => {
            let batch = ACTIVE_BATCH.load(deps.storage)?;
            let resp = batch.map(|b| BatchResponse {
                batch_id: b.batch_id,
                btc_requested: b.btc_requested.to_string(),
                collected_amount: b.collected_amount.to_string(),
                collector_historical_balance: b.collector_historical_balance.to_string(),
                maxbtc_burned: b.maxbtc_burned.to_string(),
            });
            Ok(to_json_binary(&resp)?)
        }
        QueryMsg::WithdrawingBatch {} => {
            let batch = WITHDRAWING_BATCH.load(deps.storage)?;
            let resp = batch.map(|b| BatchResponse {
                batch_id: b.batch_id,
                btc_requested: b.btc_requested.to_string(),
                collected_amount: b.collected_amount.to_string(),
                collector_historical_balance: b.collector_historical_balance.to_string(),
                maxbtc_burned: b.maxbtc_burned.to_string(),
            });
            Ok(to_json_binary(&resp)?)
        }
        QueryMsg::FinalizedBatch { batch_id } => {
            let batch = FINALIZED_BATCHES.may_load(deps.storage, batch_id)?;
            let resp = batch.map(|b| BatchResponse {
                batch_id: b.batch_id,
                btc_requested: b.btc_requested.to_string(),
                collected_amount: b.collected_amount.to_string(),
                collector_historical_balance: b.collector_historical_balance.to_string(),
                maxbtc_burned: b.maxbtc_burned.to_string(),
            });
            Ok(to_json_binary(&resp)?)
        }
        QueryMsg::ContractState {} => {
            let state = FSM.get_current_state(deps.storage)?;
            Ok(to_json_binary(&state)?)
        }
        QueryMsg::ExchangeRate {} => {
            let cfg = CONFIG.load(deps.storage)?;
            let er = get_exchange_rate(&deps, env, &cfg, None).map_err(|e| {
                StdError::generic_err(format!("failed to get_exchange_rate: {}", e))
            })?;
            Ok(to_json_binary(&er)?)
        }
    }
}

/// -----------------------------------------------------------------------------------------------
/// HELPER FUNCTIONS BELOW
/// -----------------------------------------------------------------------------------------------

/// Query the token supply for a given redemption token denom
fn query_token_supply(deps: &Deps, denom: String) -> StdResult<Uint128> {
    let resp: SupplyResponse = deps
        .querier
        .query(&QueryRequest::Bank(BankQuery::Supply { denom }))?;

    Ok(resp.amount.amount)
}

/// Query the total supply of maxBTC. At any moment, the maxBTC that is help by the liquidation
/// contract is effectively taken out of circulation, because if it hasn't been burned yet, it
/// will be pretty soon, so we decrease the total supply by that amount.
fn query_maxbtc_supply(deps: &Deps, cfg: &Config, env: &Env) -> StdResult<Uint128> {
    let bank_supply =
        query_token_supply(deps, cfg.get_maxbtc_denom(env.contract.address.to_string()))?;
    let liquidation_contract_maxbtc_balance: Uint128 = deps.querier.query_wasm_smart(
        cfg.liquidation_buffer_contract.to_string(),
        &LiquidationBufferContractQueryMsg::GetMaxBTCBalance {},
    )?;
    Ok(bank_supply - liquidation_contract_maxbtc_balance)
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

/// Creates a message to burn tokenfactory tokens from an address
fn create_tokenfactory_burn_msg(
    env: Env,
    amount: Coin,
    from_address: String,
) -> StdResult<CosmosMsg> {
    Ok(Into::<CosmosMsg>::into(MsgBurn {
        sender: env.contract.address.to_string(),
        amount: Some(BaseCoin::from(amount)),
        burn_from_address: from_address,
    }))
}

/// Creates a message instructing the liquidation buffer contract to send funds back.
fn create_liquidation_buffer_clawback_msg(
    liquidation_buffer_addr: String,
    amount: Coin,
) -> Result<CosmosMsg, ContractError> {
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: liquidation_buffer_addr,
        msg: to_json_binary(&LiquidationBufferExecuteMsg::ClawBack { amount })?,
        funds: vec![],
    });
    Ok(msg)
}

/// Creates a message instructing the collector contract to send collected funds to this contract.
fn create_collector_claim_msg(
    collector_addr: String,
    amount: Coin,
) -> Result<CosmosMsg, ContractError> {
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: collector_addr,
        msg: to_json_binary(&CollectorExecuteMsg::Claim { amount })?,
        funds: vec![],
    });
    Ok(msg)
}

/// Extracts the `batch_id` encoded in the *redemption token*’s denom.
///
/// A valid redemption denom has the layout
/// `factory/{this_contract}/redemption/batch/{batch_id}`
fn get_batch_id_from_redemption_coin(
    env: Env,
    redemption_coin: Coin,
) -> Result<u64, ContractError> {
    if !redemption_coin
        .denom
        .starts_with(format!("factory/{}/redemption/batch/", env.contract.address).as_str())
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

/// Computes or retrieves from cache the **exchange rate (ER)** between BTC
/// *assets under management* (AUM) and circulating **maxBTC**.
///
/// The formula is:
///
/// ```text
/// ER = (oracle AUM + deposit buffer + liquidation buffer BTC)
///      ------------------------------------------------------
///      (maxBTC supply + BTC requested for withdrawal
///                       – maxBTC held by liquidation buffer)
/// ```
///
/// If the denominator is zero (e.g. bootstrap state) the function returns `1`
/// to avoid division by zero.
pub(crate) fn get_exchange_rate(
    deps: &Deps,
    env: Env,
    cfg: &Config,
    deposit: Option<Uint128>,
) -> Result<Decimal, ContractError> {
    // If there is a cached exchange rate, return it
    if let Some(cached_er) = CACHED_ER.load(deps.storage)? {
        return Ok(cached_er.er);
    }

    let mut er_numerator = get_aum(deps, env.clone(), cfg)?.total();

    // In a deposit scenario, the deposit coin attached to the Deposit message
    // is added to the contract balance query result (used by get_aum() above).
    // We do not want the incoming deposit to be included in the numerator,
    // so we need to subtract it.
    if let Some(deposit_amount) = deposit {
        er_numerator -= deposit_amount;
    }

    if let Some(deposits_cap) = cfg.deposits_cap {
        if er_numerator > deposits_cap {
            return Err(ContractError::DepositCapExceeded {});
        }
    }

    let maxbtc_supply = query_maxbtc_supply(deps, cfg, &env)?;
    let active_batch = ACTIVE_BATCH
        .load(deps.storage)?
        .ok_or(ContractError::BatchStateError {})?;
    let er_denominator = maxbtc_supply + active_batch.btc_requested;

    let er = if er_denominator.is_zero() {
        Decimal::one()
    } else {
        Decimal::from_ratio(er_numerator, er_denominator)
    };

    Ok(er)
}

/// Retrieves the three BTC components that make up *assets under management*
///
/// * **Oracle AUM** – authoritative on-chain feed supplied by an oracle
/// * **Deposit buffer** – BTC sitting in the contract itself waiting to get flushed
/// * **Liquidation buffer** – BTC stored in the dedicated liquidation buffer contract
fn get_aum(deps: &Deps, env: Env, cfg: &Config) -> Result<Aum, ContractError> {
    let oracle_aum: Uint128 = deps.querier.query_wasm_smart(
        cfg.aum_oracle_contract.to_string(),
        &OracleQueryMsg::GetAUM {},
    )?;
    let deposit_buffer_balance = deps
        .querier
        .query_balance(env.contract.address, &cfg.deposit_denom)?;
    let liquidation_contract_btc_balance: Uint128 = deps.querier.query_wasm_smart(
        cfg.liquidation_buffer_contract.to_string(),
        &LiquidationBufferContractQueryMsg::GetBTCBalance {},
    )?;

    Ok(Aum {
        oracle_aum,
        deposit_buffer: deposit_buffer_balance.amount,
        liquidation_buffer_contract: liquidation_contract_btc_balance,
    })
}

/// Verifies that the current AUM does **not** exceed the optional *deposit cap*.
///
/// This check is performed in places where new deposits could push the system
/// over its configured limit.
fn check_deposit_cap(deps: &Deps, env: Env, cfg: &Config) -> Result<(), ContractError> {
    if let Some(deposits_cap) = cfg.deposits_cap {
        if get_aum(deps, env.clone(), cfg)?.total() > deposits_cap {
            return Err(ContractError::DepositCapExceeded {});
        }
    }

    Ok(())
}

/// Ensures that `recipient` is present in the optional *allow-list* for
/// deposits.
///
/// If the allow-list is **not** configured (`None`) everyone is allowed.
fn check_deposits_allowlist(
    deps: &Deps,
    cfg: &Config,
    recipient: String,
) -> Result<(), ContractError> {
    let recipient = deps.api.addr_validate(&recipient)?;
    if let Some(allowlist) = cfg.deposits_allowlist.clone() {
        for addr in allowlist {
            if addr == recipient {
                return Ok(());
            }
        }

        return Err(ContractError::Unauthorized {});
    }

    Ok(())
}
