use crate::error::ContractError;
use crate::msg::{
    AUMQueryMsg, BatchResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, LiquidationExecuteMsg,
    QueryMsg,
};
use crate::state::{
    Batch, CachedAUM, Config, ContractState, ACTIVE_BATCH, ACTIVE_BATCH_START_TIME,
    BATCH_ID_COUNTER, CACHED_AUM, CONFIG, FINALIZED_BATCHES, FSM, LAST_DEPOSIT_FLUSH_TIME,
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
        owner: deps.api.addr_validate(&msg.owner)?,
        aum_contract: deps.api.addr_validate(&msg.aum_contract)?,
        liquidation_contract: deps.api.addr_validate(&msg.liquidation_contract)?,
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
        paused: false,
        cached_aum_tolerance: msg.cached_aum_tolerance,
        cached_aum_ttl: msg.cached_aum_ttl,
    };
    CONFIG.save(deps.storage, &cfg)?;
    BATCH_ID_COUNTER.save(deps.storage, &0u64)?;
    WITHDRAWING_BATCH.save(deps.storage, &None)?;
    LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &env.block.time.seconds())?;
    ACTIVE_BATCH_START_TIME.save(deps.storage, &env.block.time.seconds())?;

    // Create the first active batch
    let new_batch_id = 1u64;
    let new_batch = Batch {
        batch_id: new_batch_id,
        btc_requested: Uint128::zero(),
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
        .add_attribute("liquidation_contract", cfg.liquidation_contract.to_string())
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
        ExecuteMsg::Deposit { recipient } => execute_deposit(deps, env, info, recipient),
        ExecuteMsg::FlushDeposits {} => execute_flush_deposits(deps, env, info),
        ExecuteMsg::Withdraw {} => execute_withdraw(deps, env, info),
        ExecuteMsg::ProcessActiveBatch {} => execute_process_active_batch(deps, env, info),
        ExecuteMsg::Claim { recipient } => execute_claim(deps, env, info, recipient),
    }
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
    let mut msgs = process_cache(deps.branch(), env.clone(), &cfg)?;

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
    let er = get_exchange_rate(&deps.as_ref(), &cfg.clone())?;

    // Adjust for deposit fee
    let fee_multiplier = Decimal::one() - cfg.deposit_fee;
    let deposit_amount = Decimal::from_atomics(deposit_coin.amount, cfg.deposit_decimals)
        .map_err(|_| ContractError::InvalidDepositAmount {})?;
    let minted = (deposit_amount * fee_multiplier) / er;

    // Mint the maxBTC to the recipient
    let mint_msg = create_tokenfactory_mint_msg(
        env,
        recipient.clone(),
        Coin {
            amount: minted.atomics(),
            denom: cfg.maxbtc_denom.clone(),
        },
    )?;
    msgs.push(mint_msg);

    // 5. Return a response
    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "deposit")
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", recipient)
        .add_attribute("minted_maxbtc", minted.atomics().to_string()))
}

/// Permissionless deposit flush
/// - checks time has passed at least deposit_flush_period
/// - if liquidation contract holds less than liquidation_buffer_share of AUM, send enough
/// - if liquidation contract holds more, request some back (it will be processed next time)
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
    let mut msgs = process_cache(deps.branch(), env.clone(), &cfg)?;

    let last_time = LAST_DEPOSIT_FLUSH_TIME.load(deps.storage)?;
    let now = env.block.time.seconds();
    if now < last_time + cfg.deposit_flush_period {
        // Not enough time has passed, do nothing
        return Ok(Response::new()
            .add_messages(msgs)
            .add_attribute("action", "flush_deposits")
            .add_attribute("status", "not_enough_time_elapsed"));
    }

    let mut contract_balance = deps
        .querier
        .query_balance(env.contract.address, &cfg.deposit_denom)?;

    if contract_balance.amount.is_zero() {
        // There have been no deposits, set last flush time to now and wait for another
        // cfg.deposit_flush_period
        LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &now)?;

        return Ok(Response::new()
            .add_attribute("action", "flush_deposits")
            .add_attribute("status", "zero_outstanding_deposits"));
    }

    FSM.go_to(deps.storage, ContractState::Flushing)?;

    let aum = query_aum(&deps.as_ref(), &cfg)?;

    // The "liquidation buffer" we want is liquidation_buffer_share * aum
    let required_buffer = (Decimal::from_atomics(aum, cfg.deposit_decimals)?
        * cfg.liquidation_buffer_share)
        .atomics();

    // The actual buffer we have in the liquidation contract
    let liquidation_contract_balance = deps
        .querier
        .query_balance(&cfg.liquidation_contract, &cfg.deposit_denom)?;

    // TODO: implement properly, after discussion with Kai. Not clear how to count this.
    // Cache the full AUM that we know of: AUM from the oracle + the current deposits buffer +
    // the liquidation contract balance.
    CACHED_AUM.save(
        deps.storage,
        &Some(CachedAUM {
            aum: aum + contract_balance.amount + liquidation_contract_balance.amount.clone(),
            timeout: now + cfg.cached_aum_ttl,
        }),
    )?;

    // If liquidation contract < required => send the difference
    if liquidation_contract_balance.amount < required_buffer {
        let mut to_send = required_buffer - liquidation_contract_balance.amount;
        // We are allowed to exhaust the deposit buffer completely.
        if to_send > contract_balance.amount {
            to_send = contract_balance.amount
        }
        // We do a bank send of the deposit_denom from this contract to the liquidation contract
        let msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.liquidation_contract.to_string(),
            amount: vec![Coin {
                denom: cfg.deposit_denom.clone(),
                amount: Uint128::from(to_send),
            }],
        });
        msgs.push(msg);
        contract_balance.amount -= to_send;
    } else {
        // If liquidation contract > required => call liquidation contract's method to send back the difference
        let to_recv = Coin {
            amount: liquidation_contract_balance.amount - required_buffer,
            denom: liquidation_contract_balance.denom.clone(),
        };
        // The returned funds will be processed next time.
        if to_recv.amount > Uint128::zero() {
            let msg = create_liquidation_rebalance_msg(
                cfg.liquidation_contract.to_string(),
                to_recv.clone(),
            )?;
            msgs.push(msg);
            contract_balance.amount += to_recv.amount;
        }
    }

    // Send what's left to the deposit pump contract, which will send it to Ethereum over IBC
    // Eureka
    if !contract_balance.amount.is_zero() {
        let msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.deposit_pump_contract.to_string(),
            amount: vec![contract_balance.clone()],
        });
        msgs.push(msg);
    }

    let resp = Response::new()
        .add_messages(msgs)
        .add_attribute("action", "flush_deposits")
        .add_attribute("sender", info.sender)
        .add_attribute("liquidation_buffer", required_buffer.to_string())
        .add_attribute("contract_balance", contract_balance.to_string());

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
    let mut msgs = process_cache(deps.branch(), env.clone(), &cfg)?;

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

    // Mint redemption tokens for the current active batch
    let active_opt = ACTIVE_BATCH.load(deps.storage)?;
    let active_batch = active_opt.ok_or(ContractError::BatchStateError {})?;

    // Mint the redemption tokens (1:1 maxBTC burned)
    let minted_redemption = amount.amount;
    ACTIVE_BATCH.save(deps.storage, &Some(active_batch.clone()))?;

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
    let msgs = process_cache(deps.branch(), env.clone(), &cfg)?;

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
    let total_redemption_supply = query_token_supply(
        &deps.as_ref(),
        cfg.get_redemption_denom(
            env.contract.address.to_string(),
            active_batch.batch_id.to_string(),
        ),
    )?;
    if total_redemption_supply.is_zero() {
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
        collected_amount: Uint128::zero(),
        paid_amount: Uint128::zero(),
        collector_historical_balance: Uint128::zero(),
    };

    // Record collector_historical_balance
    let collector_balance = deps
        .querier
        .query_balance(&cfg.collector_contract, &cfg.deposit_denom)?;
    withdrawing_batch.collector_historical_balance = collector_balance.amount;

    // Get the exchange rate (no deposit fee)
    let er = get_exchange_rate(&deps.as_ref(), &cfg.clone())?;

    // The total BTC requested = total_redemption_supply * er.
    // Note: all of our tokens have the same number of decimals as the
    // deposit denom.
    let btc_requested = er * Decimal::from_atomics(total_redemption_supply, cfg.deposit_decimals)?;
    withdrawing_batch.btc_requested = btc_requested.atomics();

    // Save it in WITHDRAWING_BATCH
    WITHDRAWING_BATCH.save(deps.storage, &Some(withdrawing_batch.clone()))?;

    // Create a new ACTIVE batch
    let mut batch_id_counter = BATCH_ID_COUNTER.load(deps.storage)?;
    batch_id_counter += 1;
    let new_active_batch = Batch {
        batch_id: batch_id_counter,
        btc_requested: Uint128::zero(),
        collected_amount: Uint128::zero(),
        paid_amount: Uint128::zero(),
        collector_historical_balance: Uint128::zero(),
    };
    ACTIVE_BATCH.save(deps.storage, &Some(new_active_batch))?;
    BATCH_ID_COUNTER.save(deps.storage, &batch_id_counter)?;
    ACTIVE_BATCH_START_TIME.save(deps.storage, &now)?;

    // TODO: implement properly, after discussion with Kai
    // Cache the full AUM that we know of: AUM from the oracle + the current deposits buffer +
    // the liquidation contract balance.
    // CACHED_AUM.save(
    //     deps.storage,
    //     &Some(CachedAUM {
    //         aum: aum + contract_balance.amount + liquidation_contract_balance.amount.clone(),
    //         timeout: now + cfg.cached_aum_ttl,
    //     }),
    // )?;

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
    let mut msgs = process_cache(deps.branch(), env.clone(), &cfg)?;

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
    let redemption_supply = query_token_supply(&deps.as_ref(), redemption_coin.denom.clone())?;
    if redemption_supply.is_zero() {
        return Err(ContractError::RedemptionSupplyMismatch {});
    }
    let user_amount_dec = Decimal::from_atomics(redemption_coin.amount, cfg.deposit_decimals)?;
    let total_supply_dec = Decimal::from_atomics(redemption_supply, cfg.deposit_decimals)?;
    let fraction = user_amount_dec / total_supply_dec;

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
                liquidation_contract: cfg.liquidation_contract.to_string(),
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

/// Query the AUM contract for the current BTC-denominated AUM.
/// In practice, you'd call `AUMContract::GetAumInBtc {}` or something similar.
fn query_aum(deps: &Deps, cfg: &Config) -> StdResult<Uint128> {
    deps.querier
        .query_wasm_smart(cfg.aum_contract.to_string(), &AUMQueryMsg::GetAUM {})
}

/// Query the token supply for a given redemption token denom
fn query_token_supply(deps: &Deps, denom: String) -> StdResult<Uint128> {
    let resp: SupplyResponse = deps
        .querier
        .query(&QueryRequest::Bank(BankQuery::Supply { denom }))?;

    Ok(resp.amount.amount)
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

/// Creates a message instructing the liquidation contract to rebalance (send funds back).
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
    if !redemption_coin.denom.starts_with(
        format!(
            "factory/{}/redemption/batch/",
            env.contract.address.to_string()
        )
        .as_str(),
    ) {
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

fn get_exchange_rate(deps: &Deps, cfg: &Config) -> Result<Decimal, ContractError> {
    let mut aum = query_aum(deps, &cfg)?;
    match CACHED_AUM.load(deps.storage)? {
        Some(cached_aum) => aum = cached_aum.aum,
        None => {}
    }

    let maxbtc_supply = query_token_supply(deps, cfg.maxbtc_denom.clone())?;
    let er = if maxbtc_supply.is_zero() {
        Decimal::one()
    } else {
        Decimal::from_ratio(aum, maxbtc_supply)
    };

    Ok(er)
}

fn process_cache(deps: DepsMut, env: Env, cfg: &Config) -> Result<Vec<CosmosMsg>, ContractError> {
    let aum = query_aum(&deps.as_ref(), &cfg)?;
    let maybe_cached_aum = CACHED_AUM.load(deps.storage)?;

    match maybe_cached_aum {
        Some(cached_aum) => {
            let fsm_state = FSM.get_current_state(deps.storage)?;

            // The cache is stale, we can not perform any operations
            if env.block.time.seconds() > cached_aum.timeout {
                return Err(ContractError::ProtocolInEmergency {});
            }

            match fsm_state {
                ContractState::Idle => {
                    // If the protocol is in idle state, we can not have a cached AUM
                    Err(ContractError::ProtocolInEmergency {})
                }
                ContractState::Flushing => {
                    // If the oracle-provided AUM is greater than what we expected, last deposit
                    // definitely came through, we can make the FLUSHING -> IDLE transition and
                    // discard the cache.
                    if aum > cached_aum.aum {
                        FSM.go_to(deps.storage, ContractState::Idle)?;
                        CACHED_AUM.save(deps.storage, &None)?;
                        return Ok(vec![]);
                    }

                    // If the cache is not stale, we need to check whether the AUM from the
                    // oracle is close enough to the cached AUM (== the previously flushed deposit
                    // batch came through); if that is the case, we can make the FLUSHING -> IDLE
                    // transition and discard the cache.
                    let cached_aum_dec =
                        Decimal::from_atomics(cached_aum.aum, cfg.deposit_decimals)?;
                    let allowed_deviation = (cached_aum_dec * cfg.cached_aum_tolerance).atomics();
                    if cached_aum.aum - aum < allowed_deviation {
                        FSM.go_to(deps.storage, ContractState::Idle)?;
                        CACHED_AUM.save(deps.storage, &None)?;
                        return Ok(vec![]);
                    }

                    // The previously flushed deposit didn't come through yet, but the cache is
                    // not stale either; no issue here, we keep using the cached AUM.
                    Ok(vec![])
                }
                ContractState::Withdrawing => {
                    // If we are in a withdrawing state, there has to be a withdrawing batch
                    let withdrawing_batch = WITHDRAWING_BATCH
                        .load(deps.storage)?
                        .ok_or(ContractError::ProtocolInEmergency {})?;

                    // Calculate the collected amount
                    let current_collector_balance = deps
                        .querier
                        .query_balance(&cfg.collector_contract, &cfg.deposit_denom)?;
                    let historical = withdrawing_batch.collector_historical_balance;
                    let collected = current_collector_balance.amount - historical;

                    // If the collected amount is less than accepted_withdrawable_percentage of requested => pause
                    let requested = withdrawing_batch.btc_requested;
                    let collected_dec =
                        Decimal::from_atomics(Uint128::from(collected), cfg.deposit_decimals)?;
                    let requested_dec =
                        Decimal::from_atomics(Uint128::from(requested), cfg.deposit_decimals)?;
                    let ratio = if requested_dec.is_zero() {
                        Decimal::one()
                    } else {
                        collected_dec / requested_dec
                    };

                    // We didn't collect the required amount yet, but the cache is not stale either;
                    // no issue here, we keep waiting.
                    if ratio < cfg.collected_tolerance {
                        return Ok(vec![]);
                    }

                    let mut msgs: Vec<CosmosMsg> = vec![];
                    if collected > requested {
                        let extra = collected - requested;
                        // send `extra` to treasury
                        let send_msg: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
                            to_address: cfg.treasury_address.to_string(),
                            amount: vec![Coin {
                                denom: cfg.deposit_denom.clone(),
                                amount: Uint128::from(extra),
                            }],
                        });
                        msgs.push(send_msg);
                    }

                    let finalized_batch = Batch {
                        batch_id: withdrawing_batch.batch_id,
                        btc_requested: withdrawing_batch.btc_requested,
                        collected_amount: collected,
                        paid_amount: Uint128::zero(),
                        collector_historical_balance: withdrawing_batch
                            .collector_historical_balance,
                    };

                    // Clear the WITHDRAWING_BATCH, clear cache, transition state
                    CACHED_AUM.save(deps.storage, &None)?;
                    WITHDRAWING_BATCH.save(deps.storage, &None)?;

                    // Save the new finalized batch to finalized map
                    FINALIZED_BATCHES.save(
                        deps.storage,
                        finalized_batch.batch_id,
                        &finalized_batch,
                    )?;
                    FSM.go_to(deps.storage, ContractState::Idle)?;

                    Ok(vec![])
                }
            }
        }
        None => Ok(vec![]),
    }
}

// - Set the caches
// - Implement actual state transitions outside of process_cache()
// - Make the collected_tolerance and cached_aum_tolerance logic the same
// - Can the protocol get stuck because we first check for stale cache? Maybe it's ok, but how do we "unstuck" it?
// - Process situation when there are no outstanding deposits
