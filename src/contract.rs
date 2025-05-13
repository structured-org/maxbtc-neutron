use crate::error::ContractError;
use crate::msg::{
    AUMQueryMsg, BatchResponse, BatchStatus, ConfigResponse, ExecuteMsg, InstantiateMsg,
    LiquidationExecuteMsg, QueryMsg,
};
use crate::state::{
    Batch, Config, ACTIVE_BATCH, BATCH_ID_COUNTER, CONFIG, FINALIZED_BATCHES,
    LAST_ACTIVE_BATCH_PROCESSED_TIME, LAST_DEPOSIT_FLUSH_TIME,
    LAST_WITHDRAWING_BATCH_FINALIZED_TIME, WITHDRAWING_BATCH,
};
use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, BankQuery, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdResult, SupplyResponse, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use neutron_std::types::cosmos::base::v1beta1::Coin as BaseCoin;
use neutron_std::types::osmosis::tokenfactory::v1beta1::{MsgBurn, MsgMint};

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
        collector_contract: deps.api.addr_validate(&msg.collector_contract)?,
        treasury_address: deps.api.addr_validate(&msg.treasury_address)?,
        deposit_denom: msg.deposit_denom,
        deposit_decimals: msg.deposit_decimals,
        maxbtc_denom: msg.maxbtc_denom,
        deposit_flush_period: msg.deposit_flush_period,
        batch_active_duration: msg.batch_active_duration,
        batch_withdrawing_duration: msg.batch_withdrawing_duration,
        accepted_withdrawable_percentage: msg.accepted_withdrawable_percentage,
        liquidation_buffer_share: msg.liquidation_buffer_share,
        deposit_fee: msg.deposit_fee,
        paused: false,
    };
    CONFIG.save(deps.storage, &cfg)?;

    BATCH_ID_COUNTER.save(deps.storage, &0u64)?;
    WITHDRAWING_BATCH.save(deps.storage, &None)?;
    LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &env.block.time.seconds())?;
    LAST_ACTIVE_BATCH_PROCESSED_TIME.save(deps.storage, &env.block.time.seconds())?;
    LAST_WITHDRAWING_BATCH_FINALIZED_TIME.save(deps.storage, &env.block.time.seconds())?;

    // Create the first active batch
    let new_batch_id = 1u64;
    let new_batch = Batch {
        batch_id: new_batch_id,
        status: BatchStatus::Active,
        btc_requested: Uint128::zero(),
        collected_amount: Uint128::zero(),
        collector_historical_balance: Uint128::zero(),
    };
    ACTIVE_BATCH.save(deps.storage, &Some(new_batch))?;
    BATCH_ID_COUNTER.save(deps.storage, &new_batch_id)?;

    Ok(Response::new()
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
            cfg.accepted_withdrawable_percentage.to_string(),
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
        ExecuteMsg::Withdraw { amount } => execute_withdraw(deps, env, info),
        ExecuteMsg::ProcessActiveBatch {} => execute_process_active_batch(deps, env, info),
        ExecuteMsg::FinalizeWithdrawingBatch {} => {
            execute_finalize_withdrawing_batch(deps, env, info)
        }
        ExecuteMsg::Claim { recipient } => execute_claim(deps, env, info, recipient),
    }
}

fn execute_deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

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

    // Calculate exchange rate = AUM / maxBTC_supply
    // If supply = 0, we treat exchange rate as 1 to avoid division by zero.
    let aum = query_aum(deps.as_ref(), &cfg)?; // BTC-denominated AUM
    let maxbtc_supply = query_token_supply(deps.as_ref(), cfg.maxbtc_denom.clone())?; // supply of maxBTC

    let er = if maxbtc_supply.is_zero() {
        Decimal::one()
    } else {
        Decimal::from_ratio(aum, maxbtc_supply)
    };

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

    // 5. Return a response
    Ok(Response::new()
        .add_message(mint_msg)
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
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

    let last_time = LAST_DEPOSIT_FLUSH_TIME.load(deps.storage)?;
    let now = env.block.time.seconds();
    if now < last_time + cfg.deposit_flush_period {
        // Not enough time has passed, do nothing
        return Ok(Response::new()
            .add_attribute("action", "flush_deposits")
            .add_attribute("status", "not_enough_time_elapsed"));
    }
    // Update flush timestamp
    LAST_DEPOSIT_FLUSH_TIME.save(deps.storage, &now)?;

    let aum = query_aum(deps.as_ref(), &cfg)?;
    // The "liquidation buffer" we want is liquidation_buffer_share * aum
    let required_buffer =
        (Decimal::from_ratio(aum, 1u128) * cfg.liquidation_buffer_share).atomics();

    // The actual buffer we have in the liquidation contract?
    let liquidation_current_balance = deps
        .querier
        .query_balance(&cfg.liquidation_contract, &cfg.deposit_denom)?;

    let mut msgs: Vec<CosmosMsg> = vec![];

    let contract_balance = deps
        .querier
        .query_balance(env.contract.address, &cfg.deposit_denom)?;
    let mut to_send = required_buffer - liquidation_current_balance.amount;

    // If liquidation contract < required => send the difference
    if liquidation_current_balance.amount < required_buffer {
        // We are allowed to exhaust the deposit buffer completely.
        if to_send > contract_balance.amount {
            to_send = contract_balance.amount
        }
        if to_send > Uint128::zero() {
            // We do a bank send of the deposit_denom from this contract to the liquidation contract
            let msg = CosmosMsg::Bank(BankMsg::Send {
                to_address: cfg.liquidation_contract.to_string(),
                amount: vec![Coin {
                    denom: cfg.deposit_denom.clone(),
                    amount: Uint128::from(to_send),
                }],
            });
            msgs.push(msg);
        }
    } else {
        // If liquidation contract > required => call liquidation contract's method to send back the difference
        let to_recv = Coin {
            amount: liquidation_current_balance.amount - required_buffer,
            denom: liquidation_current_balance.denom.clone(),
        };
        // The returned funds will be processed next time.
        if to_recv.amount > Uint128::zero() {
            let msg =
                create_liquidation_rebalance_msg(cfg.liquidation_contract.to_string(), to_recv)?;
            msgs.push(msg);
        }
    }

    // Create a message that initiates an IBC Eureka transfer to the Ethereum deposit address.
    // TODO: implement.
    // TODO: Eureka is, obviously, async. Should we handle failures? Are they even possible?
    if !contract_balance.amount.is_zero() && !(to_send == contract_balance.amount) {
        let ibc_msg = execute_ibc_transfer(
            contract_balance.clone(),
            "ethereum_custody_address".to_string(), // placeholder
            "channel-XYZ".to_string(),              // placeholder
        );
        msgs.push(ibc_msg);
    }

    let resp = Response::new()
        .add_messages(msgs)
        .add_attribute("action", "flush_deposits")
        .add_attribute("sender", info.sender)
        .add_attribute("liquidation_buffer", required_buffer.to_string())
        .add_attribute("contract_balance", contract_balance.to_string());

    Ok(resp)
}

/// User requests to withdraw maxBTC
fn execute_withdraw(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

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

    // Mint redemption tokens for the current active batch
    let active_opt = ACTIVE_BATCH.load(deps.storage)?;
    let mut active_batch = active_opt.ok_or(ContractError::BatchStateError {})?;

    if active_batch.status != BatchStatus::Active {
        return Err(ContractError::BatchStateError {});
    }

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
        .add_message(burn_msg)
        .add_message(mint_redemption_msg)
        .add_attribute("action", "withdraw")
        .add_attribute("sender", info.sender)
        .add_attribute("batch_id", active_batch.batch_id.to_string())
        .add_attribute("withdraw_amount", amount.amount.to_string());

    Ok(resp)
}

/// Permissionless call to move batch ACTIVE → WITHDRAWING if time is up
fn execute_process_active_batch(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

    let now = env.block.time.seconds();
    let last_active_time = LAST_ACTIVE_BATCH_PROCESSED_TIME.load(deps.storage)?;

    if now < last_active_time + cfg.batch_active_duration {
        return Err(ContractError::CannotProcessActiveBatchYet {});
    }

    // Ensure there is no batch in WITHDRAWING right now
    let withdrawing_opt = WITHDRAWING_BATCH.load(deps.storage)?;
    if withdrawing_opt.is_some() {
        // can't proceed
        return Ok(Response::new()
            .add_attribute("action", "process_active_batch")
            .add_attribute(
                "status",
                "cannot_move_to_withdrawing_when_already_withdrawing",
            ));
    }

    // If the active batch has no redemption tokens minted,
    // it means no one wants to withdraw, so just reset the timer and do nothing
    let active_opt = ACTIVE_BATCH.load(deps.storage)?;
    let mut active_batch = active_opt.ok_or(ContractError::BatchStateError {})?;
    let total_redemption_supply = query_token_supply(
        deps.as_ref(),
        cfg.get_redemption_denom(
            env.contract.address.to_string(),
            active_batch.batch_id.to_string(),
        ),
    )?;
    if total_redemption_supply.is_zero() {
        // reset the start_time to now, so the next cycle begins
        ACTIVE_BATCH.save(deps.storage, &Some(active_batch))?;

        // also reset the last_active_batch_processed_time
        LAST_ACTIVE_BATCH_PROCESSED_TIME.save(deps.storage, &now)?;

        return Ok(Response::new()
            .add_attribute("action", "process_active_batch")
            .add_attribute("status", "no_withdraw_requests_found"));
    }

    // Transition to WITHDRAWING
    active_batch.status = BatchStatus::Withdrawing;

    // Record collector_historical_balance
    let collector_balance = deps
        .querier
        .query_balance(&cfg.collector_contract, &cfg.deposit_denom)?;
    active_batch.collector_historical_balance = collector_balance.amount;

    // Calculate the exchange rate (no deposit fee)
    let aum = query_aum(deps.as_ref(), &cfg)?;
    let maxbtc_supply = query_token_supply(deps.as_ref(), cfg.maxbtc_denom.clone())?;
    let er = if maxbtc_supply.is_zero() {
        Decimal::one()
    } else {
        Decimal::from_ratio(aum, maxbtc_supply)
    };
    // The total BTC requested = total_redemption_supply * er
    let btc_requested = er * Decimal::from_atomics(total_redemption_supply, 0)?;
    active_batch.btc_requested = btc_requested.atomics();

    // Save it in WITHDRAWING_BATCH
    WITHDRAWING_BATCH.save(deps.storage, &Some(active_batch.clone()))?;

    // Create a new ACTIVE batch
    let mut batch_id_counter = BATCH_ID_COUNTER.load(deps.storage)?;
    batch_id_counter += 1;
    let new_batch = Batch {
        batch_id: batch_id_counter,
        status: BatchStatus::Active,
        btc_requested: Uint128::zero(),
        collected_amount: Uint128::zero(),
        collector_historical_balance: Uint128::zero(),
    };
    ACTIVE_BATCH.save(deps.storage, &Some(new_batch))?;
    BATCH_ID_COUNTER.save(deps.storage, &batch_id_counter)?;

    // Update last_active_batch_processed_time
    LAST_ACTIVE_BATCH_PROCESSED_TIME.save(deps.storage, &now)?;

    let resp = Response::new()
        .add_attribute("action", "process_active_batch")
        .add_attribute("sender", info.sender)
        .add_attribute(
            "new_withdrawing_batch_id",
            active_batch.batch_id.to_string(),
        )
        .add_attribute("btc_requested", active_batch.btc_requested.to_string());
    Ok(resp)
}

/// Permissionless call to move WITHDRAWING → FINALIZED if time is up
fn execute_finalize_withdrawing_batch(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

    let now = env.block.time.seconds();
    let last_withdrawing_time = LAST_WITHDRAWING_BATCH_FINALIZED_TIME.load(deps.storage)?;

    if now < last_withdrawing_time + cfg.batch_withdrawing_duration {
        return Err(ContractError::CannotFinalizeWithdrawingBatchYet {});
    }

    let withdrawing_opt = WITHDRAWING_BATCH.load(deps.storage)?;
    let mut withdrawing_batch = match withdrawing_opt {
        Some(b) => b,
        None => {
            // No WITHDRAWING batch, just skip
            return Ok(Response::new()
                .add_attribute("action", "finalize_withdrawing_batch")
                .add_attribute("status", "no_withdrawing_batch"));
        }
    };
    if withdrawing_batch.status != BatchStatus::Withdrawing {
        return Err(ContractError::BatchStateError {});
    }

    // 1. Calculate the collected amount
    let current_collector_balance = deps
        .querier
        .query_balance(&cfg.collector_contract, &cfg.deposit_denom)?;
    // collected = historical - current
    // According to the specification:
    // "Calculate the collected amount as (collector_historical_balance - current collector balance)"
    // This is a bit reversed from the code snippet above, but we'll follow the spec:
    let historical = withdrawing_batch.collector_historical_balance;
    let collected = if historical > current_collector_balance.amount {
        historical - current_collector_balance.amount
    } else {
        Uint128::zero()
    };

    // 2. If the collecting is less than accepted_withdrawable_percentage of requested => pause
    let requested = withdrawing_batch.btc_requested;
    let collected_dec = Decimal::from_atomics(Uint128::from(collected), 0)?;
    let requested_dec = Decimal::from_atomics(Uint128::from(requested), 0)?;
    let ratio = if requested_dec.is_zero() {
        Decimal::one()
    } else {
        collected_dec / requested_dec
    };

    if ratio < cfg.accepted_withdrawable_percentage {
        // Pause the protocol
        cfg.paused = true;
        CONFIG.save(deps.storage, &cfg)?;

        // If it's more than requested, send the extra to treasury
    } else if collected > requested {
        let extra = collected - requested;
        // send `extra` to treasury
        let send_msg: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.treasury_address.to_string(),
            amount: vec![Coin {
                denom: cfg.deposit_denom.clone(),
                amount: Uint128::from(extra),
            }],
        });
        // We'll handle that in the response
        let mut resp = Response::new().add_attribute("action", "finalize_withdrawing_batch");
        resp = resp.add_message(send_msg);
        // We'll finalize the batch below
    }

    // 3. Mark the batch as FINALIZED
    withdrawing_batch.collected_amount = Uint128::from(collected);
    withdrawing_batch.status = BatchStatus::Finalized;

    // Save to finalized map
    let batch_id = withdrawing_batch.batch_id;
    FINALIZED_BATCHES.save(deps.storage, batch_id, &withdrawing_batch)?;

    // Clear the WITHDRAWING_BATCH
    WITHDRAWING_BATCH.save(deps.storage, &None)?;

    // Update the last withdrawing batch finalized time
    LAST_WITHDRAWING_BATCH_FINALIZED_TIME.save(deps.storage, &now)?;

    let resp = Response::new()
        .add_attribute("action", "finalize_withdrawing_batch")
        .add_attribute("sender", info.sender)
        .add_attribute("batch_id", batch_id.to_string())
        .add_attribute("collected_amount", collected.to_string())
        .add_attribute("requested", requested.to_string())
        .add_attribute("pause_state", cfg.paused.to_string());

    Ok(resp)
}

/// User claims their BTC, providing redemption tokens as input
fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // 1. We parse the redemption token from info.funds or info.???
    //    Actually, in the spec, the user "attaches" the redemption tokens.
    //    For tokenfactory tokens, they'd need to do a send or burn. Another approach is
    //    the user calls "Claim" with a Send or Burn from their wallet.
    //    A simpler approach (for demonstration) is to rely on message info funds.
    //    We'll check if the user included exactly one coin with denom "redemption/batch/<id>".
    if info.funds.len() != 1 {
        return Err(ContractError::WrongRedemptionTokenOrNoFunds {});
    }
    let redemption_coin = &info.funds[0];
    if !redemption_coin.denom.starts_with("redemption/batch/") {
        return Err(ContractError::WrongRedemptionTokenOrNoFunds {});
    }
    if redemption_coin.amount.is_zero() {
        return Err(ContractError::WrongRedemptionTokenOrNoFunds {});
    }

    // Extract the batch_id
    let parts: Vec<&str> = redemption_coin.denom.split('/').collect();
    if parts.len() != 3 {
        return Err(ContractError::WrongRedemptionTokenOrNoFunds {});
    }
    let batch_id: u64 = parts[2]
        .parse()
        .map_err(|_| ContractError::WrongRedemptionTokenOrNoFunds {})?;

    // 2. Check the batch is in FINALIZED state
    let finalized_batch = FINALIZED_BATCHES.may_load(deps.storage, batch_id)?;
    let batch = match finalized_batch {
        Some(b) => b,
        None => return Err(ContractError::BatchNotFinalized {}),
    };
    if batch.status != BatchStatus::Finalized {
        return Err(ContractError::BatchNotFinalized {});
    }

    // 3. The user’s portion = collected_amount * (user_redemption_tokens / total_redemption_tokens)
    //    But we need the total supply of that redemption token. We'll do a placeholder query.
    let redemption_supply = query_token_supply(deps.as_ref(), redemption_coin.denom.clone())?;
    if redemption_supply.is_zero() {
        return Err(ContractError::RedemptionSupplyMismatch {});
    }
    let user_amount_dec = Decimal::from_atomics(redemption_coin.amount, 0)?;
    let total_supply_dec = Decimal::from_atomics(redemption_supply, 0)?;
    let fraction = user_amount_dec / total_supply_dec;

    let collected_dec = Decimal::from_atomics(batch.collected_amount, 0)?;
    let user_btc_dec = collected_dec * fraction;
    let user_btc = user_btc_dec.atomics().u128();

    // 4. Send the user’s BTC to `recipient` address
    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: recipient.clone(),
        amount: vec![Coin {
            denom: cfg.deposit_denom.clone(),
            amount: Uint128::from(user_btc),
        }],
    });

    // 5. If after this claim, the total redemption tokens = the entire supply, we can consider the batch’s redemption tokens fully claimed
    //    We'll check if user_amount == redemption_supply or we track the next claims.
    //    In a typical design, you'd burn the redemption tokens from the contract, but the user "sent" them here as funds.
    //    The user’s funds are already in the contract?
    //    We can burn them now.
    let burn_msg = create_tokenfactory_burn_msg(
        env.clone(),
        redemption_coin.clone(),
        env.contract.address.to_string(),
    )?;

    let new_total_supply = redemption_supply.checked_sub(redemption_coin.amount)?;
    // If new_total_supply == 0, we do final cleanup. We'll just keep it simple:
    // we do the burn every time. If it's the last redemption, all tokens are burned, done.
    let mut resp = Response::new()
        .add_attribute("action", "claim")
        .add_attribute("batch_id", batch_id.to_string())
        .add_attribute("user_claim_btc", user_btc.to_string());

    resp = resp.add_message(send_msg).add_message(burn_msg);

    // Because we do a final check:
    if new_total_supply.is_zero() {
        // all redemption tokens claimed for this batch
        // nothing more to do
    }

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
                accepted_withdrawable_percentage: cfg.accepted_withdrawable_percentage,
                liquidation_buffer_share: cfg.liquidation_buffer_share,
                deposit_fee: cfg.deposit_fee,
            };
            Ok(to_json_binary(&resp)?)
        }
        QueryMsg::ActiveBatch {} => {
            let batch = ACTIVE_BATCH.load(deps.storage)?;
            let resp = batch.map(|b| BatchResponse {
                batch_id: b.batch_id,
                status: b.status,
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
                status: b.status,
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
                status: b.status,
                btc_requested: b.btc_requested.to_string(),
                collected_amount: b.collected_amount.to_string(),
                collector_historical_balance: b.collector_historical_balance.to_string(),
            });
            Ok(to_json_binary(&resp)?)
        }
    }
}

///////////////////////////////////////////
// PLACEHOLDER / HELPER FUNCTIONS BELOW //
///////////////////////////////////////////

/// Query the AUM contract for the current BTC-denominated AUM.
/// In practice, you'd call `AUMContract::GetAumInBtc {}` or something similar.
fn query_aum(deps: Deps, cfg: &Config) -> StdResult<Uint128> {
    deps.querier
        .query_wasm_smart(cfg.aum_contract.to_string(), &AUMQueryMsg::GetAUM {})
}

/// Query the token supply for a given redemption token denom
fn query_token_supply(deps: Deps, denom: String) -> StdResult<Uint128> {
    let resp: SupplyResponse = deps
        .querier
        .query(&QueryRequest::Bank(BankQuery::Supply { denom }))?;

    Ok(resp.amount.amount)
}

/// Creates a submessage to mint tokenfactory tokens of `denom` and credit them to `recipient`.
/// This is chain-specific; you'll need real proto messages for your chain (e.g., `cosmos.bank.v1beta1.MsgMint`).
fn create_tokenfactory_mint_msg(env: Env, recipient: String, amount: Coin) -> StdResult<CosmosMsg> {
    Ok(Into::<CosmosMsg>::into(MsgMint {
        sender: env.contract.address.to_string(),
        amount: Some(BaseCoin::from(amount)),
        mint_to_address: recipient,
    }))
}

/// Creates a submessage to burn tokenfactory tokens from an address
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

/// Creates an IBC Eureka Transfer message to send `amount` of `denom` to `remote_address` on the channel
fn execute_ibc_transfer(amount: Coin, remote_address: String, channel_id: String) -> CosmosMsg {
    // Placeholder
    CosmosMsg::Bank(BankMsg::Send {
        to_address: format!("ibc/{}:{}", channel_id, remote_address),
        amount: vec![amount],
    })
}
