use crate::error::ContractError;
pub(crate) use crate::utils::dec_to_amount;
use cosmwasm_std::{
    entry_point, to_json_binary, Attribute, BankMsg, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    Int256, MessageInfo, QueryRequest, Response, SignedDecimal256, StdError, StdResult, Uint128,
    Uint64, WasmMsg,
};
use cw2::set_contract_version;
use cw_ownable::{assert_owner, initialize_owner};
use cw_storage_plus::Bound;
use maxbtc_base::msg::core::{
    GetAumResponse, WaitosaurObserverExecuteMsg, WaitosaurObserverQueryMsg,
};
use maxbtc_base::msg::token::ExecuteMsg as TokenExecuteMsg;
use maxbtc_base::msg::{
    core::{
        AllowlistQueryMsg, ExchangeRateProviderQueryMsg, ExecuteMsg, GetTwaerResponse,
        InstantiateMsg, MigrateMsg, QueryMsg, SimulateDepositResponse, UpdateConfigMsg,
    },
    token::QueryMsg as TokenQueryMsg,
    waitosaur_holder::{
        ExecuteMsg as WaitsaurHolderExecuteMsg, QueryMsg as WaitsaurHolderQueryMsg,
    },
};
use maxbtc_base::state::{
    core::{
        Batch, Config, ContractState, WaitosaurObserverState, ACTIVE_BATCH, BATCH_ID_COUNTER,
        CONFIG, FINALIZED_BATCHES, FSM, WITHDRAWING_BATCH,
    },
    waitosaur_holder::State as WaitosaurHolderState,
};
use neutron_std::types::osmosis::tokenfactory::v1beta1::TokenfactoryQuerier;

const CONTRACT_NAME: &str = concat!("crates.io:structured-maxbtc__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    initialize_owner(deps.storage, deps.api, Some(&msg.owner))?;

    FSM.set_initial_state(deps.storage, ContractState::Idle)?;

    if msg.deposit_cost >= Decimal::one() {
        return Err(ContractError::DepositCostTooHigh {});
    }

    // Build the Config, now with the predictable fee collector address
    let cfg = Config {
        paused: false,
        operator: deps.api.addr_validate(&msg.operator)?,
        token_contract: deps.api.addr_validate(&msg.token_contract)?,
        factory_contract: deps.api.addr_validate(&msg.factory_contract)?,
        deposit_forwarder_contract: deps.api.addr_validate(&msg.deposit_forwarder_contract)?,
        deposit_denom: msg.deposit_denom.clone(),
        deposit_decimals: msg.deposit_decimals,
        deposit_cost: msg.deposit_cost,
        deposits_cap: msg.deposits_cap,
        allowlist_contract: deps.api.addr_validate(&msg.allowlist_contract)?,
        exchange_rate_provider_contract: deps
            .api
            .addr_validate(&msg.exchange_rate_provider_contract)?,
        exchange_rate_stale_period: msg.exchange_rate_stale_period,
        // Store the predicted address in the config
        fee_collector_contract: deps.api.addr_validate(&msg.fee_collector_contract)?,
        waitosaur_observer_contract: deps.api.addr_validate(&msg.waitosaur_observer_contract)?,
        waitosaur_holder_contract: deps.api.addr_validate(&msg.waitosaur_holder_contract)?,
        withdrawal_manager_contract: deps.api.addr_validate(&msg.withdrawal_manager_contract)?,
    };
    CONFIG.save(deps.storage, &cfg)?;

    WITHDRAWING_BATCH.save(deps.storage, &None)?;

    create_new_batch(deps, &cfg)?;

    // 5. Build the final response with all necessary messages and attributes
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("allowlist_contract", cfg.allowlist_contract.to_string())
        .add_attribute("deposit_denom", cfg.deposit_denom.clone())
        .add_attribute(
            "instantiated_fee_collector_address",
            cfg.fee_collector_contract.to_string(),
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
        ExecuteMsg::Tick {} => execute_tick(deps, env, info),
        ExecuteMsg::Deposit {
            recipient,
            min_receive_amount,
        } => execute_deposit(deps, env, info, recipient, min_receive_amount),
        ExecuteMsg::MintFee { amount } => execute_mint_fee(deps, env, info, amount),
        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new().add_attribute("action", "update_ownership"))
        }
        ExecuteMsg::Withdraw {} => execute_withdraw(deps, env, info),
    }
}

pub(crate) fn execute_tick(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

    if cfg.operator != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let current_state = FSM.get_current_state(deps.storage)?;

    match current_state {
        ContractState::Idle => execute_tick_idle(deps.branch(), env, info),
        //
        ContractState::DepositNeutron => execute_tick_deposit_neutron(deps.branch()),
        //
        ContractState::DepositPending => execute_tick_deposit_pending(deps.branch()),
        ContractState::DepositJLP => execute_tick_deposit_jlp(deps.branch()),
        ContractState::WithdrawJLP => execute_tick_withdraw_jlp(deps.branch()),
        ContractState::WithdrawPending => execute_tick_withdraw_pending(deps.branch()),
        ContractState::WithdrawNeutron => execute_tick_withdraw_neutron(deps.branch()),
    }
}

fn execute_tick_withdraw_jlp(deps: DepsMut) -> Result<Response, ContractError> {
    FSM.go_to(deps.storage, ContractState::WithdrawPending)?;

    Ok(Response::new()
        .add_attribute("action", "tick")
        .add_attribute("stage", "withdraw_jlp"))
}

fn execute_tick_withdraw_pending(deps: DepsMut) -> Result<Response, ContractError> {
    let withdrawing_batch = WITHDRAWING_BATCH.load(deps.storage)?;

    let cfg = CONFIG.load(deps.storage)?;

    let mut attrs = vec![];
    let mut msgs = vec![];

    if let Some(mut withdrawing_batch) = withdrawing_batch {
        let waitosaur_holder_state = deps.querier.query_wasm_smart::<WaitosaurHolderState>(
            &cfg.waitosaur_holder_contract,
            &WaitsaurHolderQueryMsg::GetState {},
        )?;

        if let WaitosaurHolderState::Locked { amount, .. } = waitosaur_holder_state {
            withdrawing_batch.collected_amount += amount;

            attrs.push(Attribute::new(
                "received_from_binance".to_string(),
                amount.to_string(),
            ));

            // Unlock waitosaur
            let unlock_msg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.waitosaur_holder_contract.to_string(),
                msg: to_json_binary(&WaitsaurHolderExecuteMsg::Unlock {})?,
                funds: vec![],
            });

            msgs.push(unlock_msg);

            WITHDRAWING_BATCH.save(deps.storage, &Some(withdrawing_batch))?;
            FSM.go_to(deps.storage, ContractState::WithdrawNeutron)?;
        }
    }

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "tick")
        .add_attribute("stage", "withdraw_pending")
        .add_attributes(attrs))
}

fn execute_tick_withdraw_neutron(deps: DepsMut) -> Result<Response, ContractError> {
    let withdrawing_batch = WITHDRAWING_BATCH.load(deps.storage)?;

    if let Some(withdrawing_batch) = withdrawing_batch {
        FINALIZED_BATCHES.save(deps.storage, withdrawing_batch.batch_id, &withdrawing_batch)?;
        WITHDRAWING_BATCH.save(deps.storage, &None)?;
    }

    FSM.go_to(deps.storage, ContractState::Idle)?;

    Ok(Response::new()
        .add_attribute("action", "tick")
        .add_attribute("stage", "withdraw_neutron"))
}

fn execute_tick_idle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let mut active_withdraw_batch = ACTIVE_BATCH.load(deps.storage)?;
    let deposit_balance = deps
        .querier
        .query_balance(env.clone().contract.address, cfg.deposit_denom.as_str())?;

    if !active_withdraw_batch.maxbtc_burned.is_zero() {
        let exchange_rate = get_exchange_rate(&deps.as_ref(), &env, &cfg)?;
        active_withdraw_batch.btc_requested = exchange_rate
            .checked_mul(Decimal::from_atomics(
                active_withdraw_batch.maxbtc_burned,
                0,
            )?)?
            .to_uint_floor();

        active_withdraw_batch.collected_amount = active_withdraw_batch
            .btc_requested
            .min(deposit_balance.amount);

        if active_withdraw_batch.btc_requested <= deposit_balance.amount {
            FINALIZED_BATCHES.save(
                deps.storage,
                active_withdraw_batch.batch_id,
                &active_withdraw_batch,
            )?;
        } else {
            WITHDRAWING_BATCH.save(deps.storage, &Some(active_withdraw_batch.clone()))?;

            FSM.go_to(deps.storage, ContractState::WithdrawJLP)?;
        }

        let new_batch_id = create_new_batch(deps.branch(), &cfg)?;

        let send_covered_withdrawal_manager_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.withdrawal_manager_contract.to_string(),
            amount: vec![Coin {
                denom: cfg.deposit_denom,
                amount: active_withdraw_batch.collected_amount,
            }],
        });

        return Ok(Response::new()
            .add_message(send_covered_withdrawal_manager_msg)
            .add_attribute("action", "tick")
            .add_attribute("stage", "idle")
            .add_attribute("amount", active_withdraw_batch.btc_requested.to_string())
            .add_attribute(
                "covered_from_deposit",
                active_withdraw_batch.collected_amount.to_string(),
            )
            .add_attribute("new_batch_id", new_batch_id.to_string()));
    } else if !deposit_balance.amount.is_zero() {
        FSM.go_to(deps.storage, ContractState::DepositNeutron)?;
        return execute_flush_deposits(deps.branch(), env.clone(), info.clone());
    }

    Ok(Response::new()
        .add_attribute("action", "tick")
        .add_attribute("stage", "idle"))
}

fn execute_tick_deposit_neutron(deps: DepsMut) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let waitosaur_observer_state: WaitosaurObserverState = deps.querier.query_wasm_smart(
        &cfg.waitosaur_observer_contract,
        &WaitosaurObserverQueryMsg::GetState {},
    )?;

    if let WaitosaurObserverState::Locked { .. } = waitosaur_observer_state {
        return Err(ContractError::WaitosaurLocked {});
    }

    FSM.go_to(deps.storage, ContractState::DepositPending)?;

    Ok(Response::new()
        .add_attribute("action", "tick")
        .add_attribute("stage", "deposit_neutron"))
}

fn execute_tick_deposit_pending(deps: DepsMut) -> Result<Response, ContractError> {
    FSM.go_to(deps.storage, ContractState::DepositJLP)?;

    // TODO: Implement

    Ok(Response::new()
        .add_attribute("action", "tick")
        .add_attribute("stage", "deposit_pending"))
}

fn execute_tick_deposit_jlp(deps: DepsMut) -> Result<Response, ContractError> {
    FSM.go_to(deps.storage, ContractState::Idle)?;

    // TODO: Implement

    Ok(Response::new()
        .add_attribute("action", "tick")
        .add_attribute("stage", "deposit_jlp"))
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

    let maxbtc_denom = deps.querier.query_wasm_smart::<String>(
        &cfg.token_contract,
        &TokenQueryMsg::GetDenom { subdenom: None },
    )?;

    if amount.denom != maxbtc_denom {
        return Err(ContractError::InvalidMintDenom {
            expected: maxbtc_denom,
            received: amount.denom.to_string(),
        });
    }

    // Mint the maxBTC to the recipient
    let mint_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.token_contract.to_string(),
        msg: to_json_binary(&TokenExecuteMsg::Mint {
            amount: amount.clone(),
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
    if let Some(operator) = updates.operator {
        let validated_addr = deps.api.addr_validate(&operator)?;
        cfg.operator = validated_addr.clone();
        res = res.add_attribute("operator", operator.to_string());
    }
    if let Some(addr) = updates.deposit_forwarder_contract {
        let validated_addr = deps.api.addr_validate(&addr)?;
        cfg.deposit_forwarder_contract = validated_addr.clone();
        res = res.add_attribute(
            "deposit_forwarder_contract_updated",
            validated_addr.to_string(),
        );
    }
    if let Some(cap) = updates.deposits_cap {
        cfg.deposits_cap = cap;
        res = res.add_attribute("deposits_cap_updated", format!("{:?}", cap));
    }
    if let Some(deposit_cost) = updates.deposit_cost {
        if deposit_cost >= Decimal::one() {
            return Err(ContractError::DepositCostTooHigh {});
        }
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
    if let Some(addr) = updates.waitosaur_observer_contract {
        let validated_addr = deps.api.addr_validate(&addr)?;
        cfg.waitosaur_observer_contract = validated_addr.clone();
        res = res.add_attribute(
            "waitosaur_observer_contract_updated",
            validated_addr.to_string(),
        );
    }

    if let Some(addr) = updates.waitsaur_holder_contract {
        let validated_addr = deps.api.addr_validate(&addr)?;
        cfg.waitosaur_holder_contract = validated_addr.clone();
        res = res.add_attribute(
            "waitosaur_holder_contract_updated",
            validated_addr.to_string(),
        );
    }
    if let Some(addr) = updates.withdrawal_manager_contract {
        let validated_addr = deps.api.addr_validate(&addr)?;
        cfg.withdrawal_manager_contract = validated_addr.clone();
        res = res.add_attribute(
            "withdrawal_manager_contract_updated",
            validated_addr.to_string(),
        );
    }

    if let Some(value) = updates.exchange_rate_stale_period {
        if value < Uint64::one() {
            return Err(ContractError::StalePeriodMustBePositive {});
        }

        cfg.exchange_rate_stale_period = value;
        res = res.add_attribute("exchange_rate_stale_period_updated", value.to_string());
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
    let amount = cw_utils::must_pay(&info, &cfg.deposit_denom.clone())?;

    // We can't deposit if the total AUM are greater than the cap.
    check_deposit_cap(&deps.as_ref(), &cfg, Some(amount))?;
    // We can't deposit if the recipient address is not allowlisted.
    check_allowlist(&deps.as_ref(), &cfg, recipient.clone())?;

    // Calculate the amount of maxBTC to mint.
    let minted_amount = calculate_mint_amount(deps.as_ref(), env, amount)?;

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

    let maxbtc_denom = deps.querier.query_wasm_smart::<String>(
        &cfg.token_contract,
        &TokenQueryMsg::GetDenom { subdenom: None },
    )?;

    // Mint the maxBTC to the recipient
    let mint_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.token_contract.to_string(),
        msg: to_json_binary(&TokenExecuteMsg::Mint {
            amount: Coin {
                amount: minted_amount,
                denom: maxbtc_denom,
            },
            recipient: recipient.clone(),
        })?,
        funds: vec![],
    });

    // Return the response
    Ok(Response::new()
        .add_message(mint_msg)
        .add_attribute("action", "deposit")
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", recipient)
        .add_attribute("minted_maxbtc", minted_amount.to_string()))
}

/// User requests to withdraw BTC and burn their maxBTC
pub(crate) fn execute_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

    // We can't withdraw if the sender/recipient address is not allowlisted.
    check_allowlist(&deps.as_ref(), &cfg, info.sender.to_string())?;

    let mut msgs = vec![];

    let maxbtc_denom = deps.querier.query_wasm_smart::<String>(
        &cfg.token_contract,
        &TokenQueryMsg::GetDenom { subdenom: None },
    )?;

    // Input funds validation happens here
    let burned_amount = cw_utils::must_pay(&info, &maxbtc_denom.clone())?;

    // Burn the maxBTC from user
    let burn_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.token_contract.to_string(),
        msg: to_json_binary(&TokenExecuteMsg::Burn {})?,
        funds: vec![Coin {
            denom: maxbtc_denom,
            amount: burned_amount,
        }],
    });
    msgs.push(burn_msg);

    // Update the maxbtc_burned amount in the active batch
    let mut active_batch = ACTIVE_BATCH.load(deps.storage)?;
    active_batch.maxbtc_burned += burned_amount;
    ACTIVE_BATCH.save(deps.storage, &active_batch.clone())?;

    // Mint the redemption tokens (1:1 maxBTC burned)
    let minted_redemption = burned_amount;

    let redemption_subdenom = format!("redemption/batch/{}", active_batch.batch_id);
    let redemption_denom_full = deps.querier.query_wasm_smart::<String>(
        &cfg.token_contract,
        &TokenQueryMsg::GetDenom {
            subdenom: Some(redemption_subdenom.to_string()),
        },
    )?;

    let tf = TokenfactoryQuerier::new(&deps.querier);

    let denom_metadata = tf.denom_authority_metadata(
        cfg.token_contract.to_string(),
        redemption_subdenom.to_string(),
    )?;

    if denom_metadata.authority_metadata.is_none()
        || denom_metadata.authority_metadata.unwrap().admin.is_empty()
    {
        let mint_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.token_contract.to_string(),
            msg: to_json_binary(&TokenExecuteMsg::CreateRedemptionToken {
                redemption_subdenom,
            })?,
            funds: vec![],
        });

        msgs.push(mint_msg);
    }

    // Construct a message to mint redemption tokens
    let mint_redemption_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.token_contract.to_string(),
        msg: to_json_binary(&TokenExecuteMsg::Mint {
            amount: Coin {
                amount: minted_redemption,
                denom: redemption_denom_full,
            },
            recipient: info.sender.to_string(),
        })?,
        funds: vec![],
    });

    let resp = Response::new()
        .add_messages(msgs)
        .add_message(mint_redemption_msg)
        .add_attribute("action", "withdraw")
        .add_attribute("sender", info.sender)
        .add_attribute("batch_id", active_batch.batch_id.to_string())
        .add_attribute("withdraw_amount", burned_amount.to_string());

    Ok(resp)
}

/// Flushes the contract's accumulated deposit balance to the deposit forwarder contract.
///
/// This handler can be triggered by any account, but its execution is rate-limited
/// by the `deposit_flush_period` defined in the contract's configuration.
fn execute_flush_deposits(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // check if state is not in DepositNeutron
    let current_state = FSM.get_current_state(deps.storage)?;
    if current_state != ContractState::DepositNeutron {
        return Err(ContractError::FlushDepositAllowedInDepositNeutron {});
    }

    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::ContractPaused {});
    }

    let amount_to_flush = deps
        .querier
        .query_balance(env.contract.address, cfg.deposit_denom.as_str())?;

    let mut msgs = vec![];

    // Send what's left to the deposit forwarder contract, which will send it to Ethereum over IBC
    // Eureka though the valence library + base account combo
    if !amount_to_flush.amount.is_zero() {
        let msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.deposit_forwarder_contract.to_string(),
            amount: vec![Coin {
                denom: cfg.deposit_denom,
                amount: amount_to_flush.amount,
            }],
        });
        msgs.push(msg);
    }

    // Set waitosaur observer lock with the amount to flush
    let lock_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.waitosaur_observer_contract.to_string(),
        msg: to_json_binary(&WaitosaurObserverExecuteMsg::Lock {
            amount: SignedDecimal256::from(Decimal::from_atomics(amount_to_flush.amount, 0)?),
        })?,
        funds: vec![],
    });

    msgs.push(lock_msg);

    let resp = Response::new()
        .add_messages(msgs)
        .add_attribute("action", "flush_deposits")
        .add_attribute("sender", info.sender)
        .add_attribute("flushed", amount_to_flush.to_string());

    Ok(resp)
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::ContractState {} => Ok(to_json_binary(&FSM.get_current_state(deps.storage)?)?),
        QueryMsg::Config {} => {
            let cfg = CONFIG.load(deps.storage)?;
            Ok(to_json_binary(&cfg)?)
        }
        QueryMsg::ActiveBatch {} => {
            let active_batch = ACTIVE_BATCH.load(deps.storage)?;
            Ok(to_json_binary(&active_batch)?)
        }
        QueryMsg::WithdrawingBatch {} => {
            let withdrawing_batch = WITHDRAWING_BATCH.load(deps.storage)?;
            Ok(to_json_binary(&withdrawing_batch)?)
        }
        QueryMsg::FinalizedBatch { batch_id } => {
            let finalized_batch = FINALIZED_BATCHES.load(deps.storage, batch_id)?;
            Ok(to_json_binary(&finalized_batch)?)
        }
        QueryMsg::FinalizedBatches { start_after, limit } => {
            let bound = start_after.map(Bound::exclusive);
            let finalized_batches = FINALIZED_BATCHES
                .range(deps.storage, bound, None, cosmwasm_std::Order::Ascending)
                .map(|item| {
                    item.map(|(_, batch)| batch)
                        .map_err(|e| StdError::generic_err(e.to_string()))
                })
                .take(limit.unwrap_or(10).min(100) as usize)
                .collect::<StdResult<Vec<Batch>>>()?;

            Ok(to_json_binary(&finalized_batches)?)
        }
        QueryMsg::ExchangeRate {} => {
            let cfg = CONFIG.load(deps.storage)?;
            let er = get_exchange_rate(&deps, &env, &cfg)
                .map_err(|e| StdError::generic_err(format!("failed to get_exchange_rate: {e}")))?;
            Ok(to_json_binary(&er)?)
        }
        QueryMsg::SimulateDeposit { amount } => {
            // Call the dedicated calculation function and map its error type to StdError
            let minted_amount = calculate_mint_amount(deps, env, amount)
                .map_err(|e| StdError::generic_err(format!("Calculation failed: {e}")))?;

            let resp = SimulateDepositResponse { minted_amount };
            to_json_binary(&resp)
        }
        QueryMsg::Ownership {} => Ok(to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?),
    }
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
        cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    }

    Ok(Response::new())
}

/* -----------------------------------------------------------------------------------------------
/ HELPER FUNCTIONS BELOW
/ -----------------------------------------------------------------------------------------------*/

/// Creates new batch and returns batch_id
fn create_new_batch(deps: DepsMut, cfg: &Config) -> Result<u64, ContractError> {
    let new_batch_id = BATCH_ID_COUNTER.load(deps.storage).unwrap_or(0u64) + 1;

    let new_batch = Batch {
        batch_id: new_batch_id,
        btc_requested: Uint128::zero(),
        maxbtc_burned: Uint128::zero(),
        collected_amount: Uint128::zero(),
        deposit_decimals: cfg.deposit_decimals,
        collector_historical_balance: Uint128::zero(),
    };
    ACTIVE_BATCH.save(deps.storage, &new_batch)?;
    BATCH_ID_COUNTER.save(deps.storage, &new_batch_id)?;

    Ok(new_batch_id)
}
/// Calculates the amount of maxBTC to be minted for a given deposit amount.
/// This function encapsulates the core logic used in both deposits and simulations.
fn calculate_mint_amount(
    deps: Deps,
    env: Env,
    deposit_amount_raw: Uint128,
) -> Result<Uint128, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // Get the current exchange rate
    let er = get_exchange_rate(&deps, &env, &cfg)?;

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
pub(crate) fn get_exchange_rate(
    deps: &Deps,
    env: &Env,
    cfg: &Config,
) -> Result<Decimal, ContractError> {
    let res: GetTwaerResponse =
        deps.querier
            .query(&QueryRequest::Wasm(cosmwasm_std::WasmQuery::Smart {
                contract_addr: cfg.exchange_rate_provider_contract.to_string(),
                msg: to_json_binary(&ExchangeRateProviderQueryMsg::GetTwaer {})?,
            }))?;

    if Uint64::from(env.block.time.seconds() - res.published_at) > cfg.exchange_rate_stale_period {
        return Err(ContractError::ERDataStale {});
    }

    Ok(res.twaer)
}

/// Queries the AUM from the twaer provider contract.
pub(crate) fn get_aum(deps: &Deps, cfg: &Config) -> Result<Int256, ContractError> {
    let res: GetAumResponse =
        deps.querier
            .query(&QueryRequest::Wasm(cosmwasm_std::WasmQuery::Smart {
                contract_addr: cfg.exchange_rate_provider_contract.to_string(),
                msg: to_json_binary(&ExchangeRateProviderQueryMsg::GetAum {})?,
            }))?;

    Ok(res.aum_in_wbtc)
}

/// Verifies that the current Deposits does **not** exceed the optional *deposit cap*.
fn check_deposit_cap(
    deps: &Deps,
    cfg: &Config,
    deposit: Option<Uint128>,
) -> Result<(), ContractError> {
    if let Some(deposits_cap) = cfg.deposits_cap {
        let current_aum = get_aum(deps, cfg)?;
        if current_aum + Int256::from(deposit.unwrap_or_default()) > deposits_cap.into() {
            return Err(ContractError::DepositCapExceeded {});
        }
    }

    Ok(())
}

/// Ensures that `user` is present in the *allow-list* by querying
/// the allow-list contract.
fn check_allowlist(deps: &Deps, cfg: &Config, user: String) -> Result<(), ContractError> {
    deps.api.addr_validate(&user)?;
    let is_allowed: bool =
        deps.querier
            .query(&QueryRequest::Wasm(cosmwasm_std::WasmQuery::Smart {
                contract_addr: cfg.allowlist_contract.to_string(),
                msg: to_json_binary(&AllowlistQueryMsg::IsAddressAllowed { address: user })?,
            }))?;
    if !is_allowed {
        Err(ContractError::AddressNotAllowed {})
    } else {
        Ok(())
    }
}
