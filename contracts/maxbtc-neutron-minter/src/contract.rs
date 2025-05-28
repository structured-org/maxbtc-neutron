use crate::error::ContractError;
use crate::msg::{
    BatchResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, LiquidationBufferContractQueryMsg,
    LiquidationExecuteMsg, OracleQueryMsg, QueryMsg, UpdateConfigMsg,
};
use crate::state::{
    Batch, CachedAUM, CachedER, Config, ContractState, ACTIVE_BATCH, ACTIVE_BATCH_START_TIME,
    BATCH_ID_COUNTER, CACHED_ER, CONFIG, FINALIZED_BATCHES, FSM, LAST_DEPOSIT_FLUSH_TIME,
    WITHDRAWING_BATCH,
};
use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, BankQuery, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdResult, SupplyResponse, Uint128, WasmMsg,
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

    let cfg = Config {
        paused: false,
        owner: deps.api.addr_validate(&msg.owner)?,
        aum_contract: deps.api.addr_validate(&msg.aum_contract)?,
        liquidation_buffer_contract: deps.api.addr_validate(&msg.liquidation_contract)?,
        deposit_pump_contract: deps.api.addr_validate(&msg.deposit_pump_contract)?,
        collector_contract: deps.api.addr_validate(&msg.collector_contract)?,
        treasury_address: deps.api.addr_validate(&msg.treasury_address)?,
        deposit_denom: msg.deposit_denom,
        deposit_decimals: msg.deposit_decimals,
        maxbtc_denom: msg.maxbtc_denom,
        deposit_flush_period: msg.deposit_flush_period,
        batch_active_duration: msg.batch_active_duration,
        batch_withdrawing_duration: msg.batch_withdrawing_duration,
        collected_tolerance: msg.accepted_withdrawable_percentage,
        liquidation_buffer_share: msg.liquidation_buffer_share,
        deposit_fee: msg.deposit_fee,
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
    };
    CONFIG.save(deps.storage, &cfg)?;
    BATCH_ID_COUNTER.save(deps.storage, &0u64)?;
    WITHDRAWING_BATCH.save(deps.storage, &None)?;
    LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &env.block.time.seconds())?;
    ACTIVE_BATCH_START_TIME.save(deps.storage, &env.block.time.seconds())?;
    CACHED_ER.save(deps.storage, &None)?;

    // Create the first active batch
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

    // Create the maxBTC denom
    let create_maxbtc_denom_msg =
        create_tokenfactory_create_denom_msg(env.clone(), cfg.maxbtc_denom.clone())?;

    // Initialise the FSM in the Idle state
    FSM.set_initial_state(deps.storage, ContractState::Idle)?;

    Ok(Response::new()
        .add_message(create_maxbtc_denom_msg)
        .add_attribute("action", "instantiate")
        .add_attribute("owner", cfg.owner.to_string())
        .add_attribute("aum_contract", cfg.aum_contract.to_string())
        .add_attribute(
            "liquidation_buffer_contract",
            cfg.liquidation_buffer_contract.to_string(),
        )
        .add_attribute("collector_contract", cfg.collector_contract.to_string())
        .add_attribute("treasury_address", cfg.treasury_address.to_string())
        .add_attribute("deposit_denom", cfg.deposit_denom.clone())
        .add_attribute("maxbtc_denom", cfg.maxbtc_denom.clone())
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
        .add_attribute("deposit_fee", cfg.deposit_fee.to_string()))
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
    }
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
        cfg.aum_contract = deps.api.addr_validate(&addr)?;
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
    if let Some(v) = updates.deposit_fee {
        cfg.deposit_fee = v;
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

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
}

fn execute_deposit(
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

    // Validate funds
    if info.funds.is_empty() {
        return Err(ContractError::NoFundsSent {});
    }
    if info.funds.len() != 1 {
        return Err(ContractError::InvalidDepositAmount {});
    }
    let deposit_coin = &info.funds[0];
    if deposit_coin.denom != cfg.deposit_denom {
        return Err(ContractError::InvalidDepositDenom {
            expected: cfg.deposit_denom,
            received: deposit_coin.denom.clone(),
        });
    }
    if deposit_coin.amount.is_zero() {
        return Err(ContractError::InvalidDepositAmount {});
    }

    // Get the exchange rate
    let er = get_exchange_rate(&deps.as_ref(), env.clone(), &cfg.clone())?;

    // Adjust for deposit fee
    let fee_multiplier = Decimal::one() - cfg.deposit_fee;
    let deposit_amount = Decimal::from_atomics(deposit_coin.amount, cfg.deposit_decimals.into())
        .map_err(|_| ContractError::InvalidDepositAmount {})?;

    // Apply the deposit fee and divide by exchange rate
    let fee_multiplier = Decimal::one() - cfg.deposit_fee;
    let minted_dec = (deposit_amount * fee_multiplier) / er;

    // CosmWasm's Decimal always uses 18 digits internally, so scale down
    // to match cfg.deposit_decimals (e.g., 6) before minting.
    let minted_amount = minted_dec
        .atomics()
        .checked_div(Uint128::from(
            10u128.pow(minted_dec.decimal_places() - cfg.deposit_decimals),
        ))
        .map_err(|_| ContractError::InvalidDepositAmount {})?;

    // 4. Mint the maxBTC to the recipient
    let mint_msg = create_tokenfactory_mint_msg(
        env,
        recipient.clone(),
        Coin {
            amount: minted_amount,
            denom: cfg.maxbtc_denom.clone(),
        },
    )?;
    msgs.push(mint_msg);

    // 5. Return the response
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
fn execute_flush_deposits(
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

    let mut deposit_buffer = deps
        .querier
        .query_balance(env.contract.address.clone(), &cfg.deposit_denom)?;

    if deposit_buffer.amount.is_zero() {
        // There have been no deposits, set last flush time to now and wait for another
        // cfg.deposit_flush_period
        LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &now)?;

        return Ok(Response::new()
            .add_attribute("action", "flush_deposits")
            .add_attribute("status", "zero_outstanding_deposits"));
    }

    FSM.go_to(deps.storage, ContractState::Flushing)?;

    let oracle_aum = deps
        .querier
        .query_wasm_smart(cfg.aum_contract.to_string(), &OracleQueryMsg::GetAUM {})?;

    // The "liquidation buffer" we want is liquidation_buffer_share * oracle_aum
    let required_buffer = (Decimal::from_atomics(oracle_aum, cfg.deposit_decimals)?
        * cfg.liquidation_buffer_share)
        .atomics();

    // The actual buffer we have in the liquidation buffer contract
    let liquidation_contract_balance: Uint128 = deps.querier.query_wasm_smart(
        cfg.liquidation_buffer_contract.to_string(),
        &LiquidationBufferContractQueryMsg::GetMaxBTCBalance {},
    )?;

    // Cache the exchange rate and oracle_aum + deposit_buffer value, because we need it
    // in process_cache() to check whether the deposit reached Binance / Solana
    let er = get_exchange_rate(&deps.as_ref(), env.clone(), &cfg)?;
    CACHED_ER.save(
        deps.storage,
        &Some(CachedER {
            er,
            timeout: now + cfg.cached_er_ttl,
            aum: Some(CachedAUM {
                oracle_aum,
                deposit_buffer: deposit_buffer.amount,
            }),
        }),
    )?;

    // If liquidation buffer contract < required => send the difference
    if liquidation_contract_balance < required_buffer {
        let mut to_send = required_buffer - liquidation_contract_balance;
        // We are allowed to exhaust the deposit buffer completely.
        if to_send > deposit_buffer.amount {
            to_send = deposit_buffer.amount
        }
        // We do a bank send of the deposit_denom from this contract to the liquidation buffer contract
        let msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.liquidation_buffer_contract.to_string(),
            amount: vec![Coin {
                denom: cfg.deposit_denom.clone(),
                amount: to_send,
            }],
        });
        msgs.push(msg);
        deposit_buffer.amount -= to_send;
    } else {
        // If liquidation buffer contract > required => call liquidation buffer contract's method to
        // send back the difference
        let to_recv = Coin {
            amount: liquidation_contract_balance - required_buffer,
            denom: cfg.deposit_denom.clone(),
        };
        // The returned funds will be processed next time.
        if to_recv.amount > Uint128::zero() {
            let msg = create_liquidation_rebalance_msg(
                cfg.liquidation_buffer_contract.to_string(),
                to_recv.clone(),
            )?;
            msgs.push(msg);
            deposit_buffer.amount += to_recv.amount;
        }
    }

    // Send what's left to the deposit pump contract, which will send it to Ethereum over IBC
    // Eureka
    if !deposit_buffer.amount.is_zero() {
        let msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.deposit_pump_contract.to_string(),
            amount: vec![deposit_buffer.clone()],
        });
        msgs.push(msg);
    }

    let resp = Response::new()
        .add_messages(msgs)
        .add_attribute("action", "flush_deposits")
        .add_attribute("sender", info.sender)
        .add_attribute("liquidation_buffer", required_buffer.to_string())
        .add_attribute("contract_balance", deposit_buffer.to_string());

    Ok(resp)
}

/// User requests to withdraw BTC and burn their maxBTC
fn execute_withdraw(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }
    let mut msgs = _process_cache(deps.branch(), env.clone(), &cfg)?;

    // Validate that we received some MaxBTC
    if info.funds.is_empty() {
        return Err(ContractError::NoFundsSent {});
    }
    if info.funds.len() != 1 {
        return Err(ContractError::InvalidDepositAmount {});
    }
    let amount = &info.funds[0];
    if amount.denom != cfg.get_maxbtc_denom(env.contract.address.to_string()) {
        return Err(ContractError::InvalidDepositDenom {
            expected: cfg.deposit_denom,
            received: amount.denom.clone(),
        });
    }
    if amount.amount.is_zero() {
        return Err(ContractError::InvalidDepositAmount {});
    }

    // Burn the maxBTC from user
    let burn_msg =
        create_tokenfactory_burn_msg(env.clone(), amount.clone(), info.sender.to_string())?;
    msgs.push(burn_msg);

    // Update the maxbtc_burned amount in the active batch
    let mut active_batch = ACTIVE_BATCH
        .load(deps.storage)?
        .ok_or(ContractError::BatchStateError {})?;
    active_batch.maxbtc_burned += amount.amount;
    ACTIVE_BATCH.save(deps.storage, &Some(active_batch.clone()))?;

    // Mint the redemption tokens (1:1 maxBTC burned)
    let minted_redemption = amount.amount;
    // Construct a message to mint redemption tokens
    let redemption_denom = format!("redemption/batch/{}", active_batch.batch_id);
    let mint_redemption_msg = create_tokenfactory_mint_msg(
        env,
        info.sender.to_string(),
        Coin {
            amount: minted_redemption,
            denom: redemption_denom.clone(),
        },
    )?;

    let resp = Response::new()
        .add_messages(msgs)
        .add_message(mint_redemption_msg)
        .add_attribute("action", "withdraw")
        .add_attribute("sender", info.sender)
        .add_attribute("batch_id", active_batch.batch_id.to_string())
        .add_attribute("withdraw_amount", amount.amount.to_string());

    Ok(resp)
}

/// Permissionless call to move batch ACTIVE → WITHDRAWING if time is up
fn execute_process_active_batch(
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
        return Err(ContractError::CannotProcessActiveBatchYet {});
    }

    // If the active batch has no redemption tokens minted,
    // it means no one wants to withdraw, so just reset the timer and do nothing
    let active_batch = ACTIVE_BATCH
        .load(deps.storage)?
        .ok_or(ContractError::BatchStateError {})?;
    let redemption_token_supply = query_token_supply(
        &deps.as_ref(),
        cfg.get_redemption_denom(
            env.contract.address.to_string(),
            active_batch.batch_id.to_string(),
        ),
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
    let er = get_exchange_rate(&deps.as_ref(), env.clone(), &cfg.clone())?;

    // The total BTC requested = total_redemption_supply * er.
    // Note: all of our tokens have the same number of decimals as the
    // deposit denom.
    let btc_requested = er * Decimal::from_atomics(redemption_token_supply, cfg.deposit_decimals)?;
    withdrawing_batch.btc_requested = btc_requested.atomics();

    // Save it in WITHDRAWING_BATCH
    WITHDRAWING_BATCH.save(deps.storage, &Some(withdrawing_batch.clone()))?;

    // Create a new ACTIVE batch
    let mut batch_id_counter = BATCH_ID_COUNTER.load(deps.storage)?;
    batch_id_counter += 1;
    let new_active_batch = Batch {
        batch_id: batch_id_counter,
        btc_requested: Uint128::zero(),
        maxbtc_burned: withdrawing_batch.maxbtc_burned,
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
fn execute_claim(
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
    let user_btc_dec = available_dec * fraction;
    let user_btc = user_btc_dec.atomics();

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

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::Config {} => {
            let cfg = CONFIG.load(deps.storage)?;
            let resp = ConfigResponse {
                owner: cfg.owner.to_string(),
                aum_contract: cfg.aum_contract.to_string(),
                liquidation_contract: cfg.liquidation_buffer_contract.to_string(),
                treasury_address: cfg.treasury_address.to_string(),
                deposit_denom: cfg.deposit_denom,
                maxbtc_denom: cfg.maxbtc_denom,
                deposit_flush_period: cfg.deposit_flush_period,
                batch_active_duration: cfg.batch_active_duration,
                batch_withdrawing_duration: cfg.batch_withdrawing_duration,
                accepted_withdrawable_percentage: cfg.collected_tolerance,
                liquidation_buffer_share: cfg.liquidation_buffer_share,
                deposit_fee: cfg.deposit_fee,
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
            });
            Ok(to_json_binary(&resp)?)
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
fn query_maxbtc_supply(deps: &Deps, cfg: &Config) -> StdResult<Uint128> {
    let bank_supply = query_token_supply(deps, cfg.maxbtc_denom.clone())?;
    let liquidation_contract_maxbtc_balance: Uint128 = deps.querier.query_wasm_smart(
        cfg.liquidation_buffer_contract.to_string(),
        &LiquidationBufferContractQueryMsg::GetMaxBTCBalance {},
    )?;
    Ok(bank_supply - liquidation_contract_maxbtc_balance)
}

/// Creates a message to mint tokenfactory tokens of `denom` and credit them to `recipient`.
fn create_tokenfactory_mint_msg(env: Env, recipient: String, amount: Coin) -> StdResult<CosmosMsg> {
    Ok(Into::<CosmosMsg>::into(MsgMint {
        sender: env.contract.address.to_string(),
        amount: Some(BaseCoin::from(amount)),
        mint_to_address: recipient,
    }))
}

/// Creates a message to create a tokenfactory denom.
fn create_tokenfactory_create_denom_msg(env: Env, denom: String) -> StdResult<CosmosMsg> {
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

/// Creates a message instructing the liquidation buffer contract to rebalance (send funds back).
fn create_liquidation_rebalance_msg(
    liquidation_addr: String,
    amount: Coin,
) -> Result<CosmosMsg, ContractError> {
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: liquidation_addr,
        msg: to_json_binary(&LiquidationExecuteMsg::ClawBack { amount })?,
        funds: vec![],
    });
    Ok(msg)
}

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

fn get_exchange_rate(deps: &Deps, env: Env, cfg: &Config) -> Result<Decimal, ContractError> {
    // If there is a cached exchange rate, return it
    if let Some(cached_er) = CACHED_ER.load(deps.storage)? {
        return Ok(cached_er.er);
    }

    // Calculate the ER as:
    // (AUM received from oracle + deposits buffer + liquidation buffer contract balance)
    //  /
    // (maxBTC total supply + maxBTC burned in current active withdrawal batch - maxBTC still owned
    // by the liquidation buffer contract)
    let er_numerator = get_aum(deps, env.clone(), cfg)?;

    if let Some(deposits_cap) = cfg.deposits_cap {
        if er_numerator > deposits_cap {
            return Err(ContractError::DepositCapExceeded {});
        }
    }

    let maxbtc_supply = query_maxbtc_supply(deps, cfg)?;
    let active_batch = ACTIVE_BATCH
        .load(deps.storage)?
        .ok_or(ContractError::BatchStateError {})?;
    let liquidation_contract_maxbtc_balance: Uint128 = deps.querier.query_wasm_smart(
        cfg.liquidation_buffer_contract.to_string(),
        &LiquidationBufferContractQueryMsg::GetMaxBTCBalance {},
    )?;
    let er_denominator =
        maxbtc_supply + active_batch.btc_requested - liquidation_contract_maxbtc_balance;

    let er = if er_denominator.is_zero() {
        Decimal::one()
    } else {
        Decimal::from_ratio(er_numerator, er_denominator)
    };

    Ok(er)
}

fn get_aum(deps: &Deps, env: Env, cfg: &Config) -> Result<Uint128, ContractError> {
    let oracle_aum: Uint128 = deps
        .querier
        .query_wasm_smart(cfg.aum_contract.to_string(), &OracleQueryMsg::GetAUM {})?;
    let deposit_buffer = deps
        .querier
        .query_balance(env.contract.address, &cfg.deposit_denom)?;
    let liquidation_contract_btc_balance: Uint128 = deps.querier.query_wasm_smart(
        cfg.liquidation_buffer_contract.to_string(),
        &LiquidationBufferContractQueryMsg::GetBTCBalance {},
    )?;

    Ok(oracle_aum + deposit_buffer.amount + liquidation_contract_btc_balance)
}

fn check_deposit_cap(deps: &Deps, env: Env, cfg: &Config) -> Result<(), ContractError> {
    if let Some(deposits_cap) = cfg.deposits_cap {
        if get_aum(deps, env.clone(), cfg)? > deposits_cap {
            return Err(ContractError::DepositCapExceeded {});
        }
    }

    Ok(())
}

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
fn _process_cache(deps: DepsMut, env: Env, cfg: &Config) -> Result<Vec<CosmosMsg>, ContractError> {
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
    let current_oracle_aum: Uint128 = deps
        .querier
        .query_wasm_smart(cfg.aum_contract.to_string(), &OracleQueryMsg::GetAUM {})?;

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
    let accepted_diff = (cached_deposit_buffer_dec * cfg.deposit_buffer_tolerance).atomics();
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

    let collected = current_collector_balance.amount - historical_collector_balance;
    let requested_dec =
        Decimal::from_atomics(withdrawing_batch.btc_requested, cfg.deposit_decimals)?;
    let accepted_diff = (requested_dec * cfg.collected_tolerance).atomics();

    // If we collected more than requested, or within the `accepted_diff` less than requested,
    // finalize the batch and transition WITHDRAWING -> IDLE.
    if collected > withdrawing_batch.btc_requested
        || withdrawing_batch.btc_requested - collected > accepted_diff
    {
        if collected > withdrawing_batch.btc_requested {
            let extra = collected - withdrawing_batch.btc_requested;
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

// - Implement checking that the liquidation buffer did, in fact, return the clawed back assets.
// - Fix decimal issues throughout the code