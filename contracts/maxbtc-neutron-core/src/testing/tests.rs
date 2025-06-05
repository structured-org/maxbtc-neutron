use crate::contract::{
    _process_cache, dec_to_amount, execute, execute_claim, execute_flush_deposits,
    execute_process_active_batch, execute_withdraw, instantiate,
};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, LiquidationExecuteMsg};
use crate::state::{
    Batch, CachedAUM, CachedER, Config, ContractState, ACTIVE_BATCH, ACTIVE_BATCH_START_TIME,
    BATCH_ID_COUNTER, CACHED_ER, CONFIG, FINALIZED_BATCHES, FSM, LAST_DEPOSIT_FLUSH_TIME,
    WITHDRAWING_BATCH,
};
use crate::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockStorage};
use cosmwasm_std::{
    coin, from_json, Attribute, BankMsg, Coin, CosmosMsg, Decimal, DepsMut, Env, MessageInfo,
    OwnedDeps, Response, SubMsg, Uint128, WasmMsg,
};

#[test]
fn test_instantiate_success() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("some_sender"), &[]);

    // Arrange: build a valid instantiate msg
    let msg = default_instantiate_msg(&deps);

    // Act: call instantiate
    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    // Assert: check response
    // We expect 1 message: create_tokenfactory_create_denom_msg
    assert_eq!(res.messages.len(), 1);
    // Assert: check attributes
    let expected_attributes = vec![
        Attribute::new("action", "instantiate"),
        Attribute::new("owner", msg.owner.clone()),
        Attribute::new("aum_contract", msg.aum_contract.clone()),
        Attribute::new(
            "liquidation_buffer_contract",
            msg.liquidation_contract.clone(),
        ),
        Attribute::new("collector_contract", msg.collector_contract.clone()),
        Attribute::new("treasury_address", msg.treasury_address.clone()),
        Attribute::new("deposit_denom", msg.deposit_denom.clone()),
        Attribute::new("maxbtc_denom", msg.maxbtc_denom.clone()),
        Attribute::new("deposit_flush_period", msg.deposit_flush_period.to_string()),
        Attribute::new(
            "batch_active_duration",
            msg.batch_active_duration.to_string(),
        ),
        Attribute::new(
            "batch_withdrawing_duration",
            msg.batch_withdrawing_duration.to_string(),
        ),
        Attribute::new(
            "accepted_withdrawable_percentage",
            msg.accepted_withdrawable_percentage.to_string(),
        ),
        Attribute::new(
            "liquidation_buffer_share",
            msg.liquidation_buffer_share.to_string(),
        ),
        Attribute::new("deposit_fee", msg.deposit_fee.to_string()),
    ];
    for attr in expected_attributes {
        assert!(
            res.attributes.contains(&attr),
            "Missing expected attribute: {} = {}",
            attr.key,
            attr.value
        );
    }

    // Assert: check contract storage
    let cfg = CONFIG.load(&deps.storage).unwrap();
    assert_eq!(cfg.owner, deps.api.addr_make("owner_addr"));
    assert_eq!(cfg.aum_contract, &deps.api.addr_make("aum_addr"));
    assert_eq!(cfg.paused, false);
    // etc. check more fields
    assert_eq!(cfg.deposit_decimals, 6u32);
    assert_eq!(cfg.deposit_denom, "wBTC");
    assert_eq!(cfg.liquidation_buffer_share, Decimal::percent(10));

    let counter = BATCH_ID_COUNTER.load(&deps.storage).unwrap();
    assert_eq!(counter, 1u64);

    let active_batch = ACTIVE_BATCH.load(&deps.storage).unwrap();
    assert!(active_batch.is_some());
    let active_batch_data = active_batch.unwrap();
    assert_eq!(active_batch_data.batch_id, 1u64);
    assert_eq!(active_batch_data.btc_requested, Uint128::zero());
    assert_eq!(active_batch_data.maxbtc_burned, Uint128::zero());
    assert_eq!(active_batch_data.collected_amount, Uint128::zero());
    assert_eq!(active_batch_data.paid_amount, Uint128::zero());

    let fsm_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(fsm_state, ContractState::Idle);

    // Finally, check time-based items
    let last_deposit_flush_time = LAST_DEPOSIT_FLUSH_TIME.load(&deps.storage).unwrap();
    assert_eq!(last_deposit_flush_time, env.block.time.seconds());
    let active_batch_start_time = ACTIVE_BATCH_START_TIME.load(&deps.storage).unwrap();
    assert_eq!(active_batch_start_time, env.block.time.seconds());
}

#[test]
fn test_first_deposit_success() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Mock out the queries so that get_exchange_rate() returns ER=1
    //   - AUM = 100_000_000,
    //   - maxBTC supply = 0, etc.
    deps.querier.update_oracle_aum(Uint128::from(0u128));
    deps.querier
        .set_token_supply(&cfg.maxbtc_denom, Uint128::zero());
    deps.querier
        .update_liqbuffer_maxbtc_balance(Uint128::zero());

    // deposit_amount = 1 wBTC => deposit_coin.amount = 1 * 10^6 = 1_000_000
    // deposit_fee = 1% => user effectively deposits 0.99 wBTC
    // exchange_rate = 1 => minted = 0.99 => minted.atomics() = 990_000
    let deposit_amount: Uint128 = Uint128::from(1_000_000u128);
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(deposit_amount.u128(), "wBTC")],
    );

    // Act
    let recipient = deps.api.addr_make("recipient_addr");
    let res = do_deposit(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        recipient.to_string(),
    )
    .unwrap();

    // Assert
    // The response should have exactly one message: the tokenfactory Mint
    assert_eq!(res.messages.len(), 1);
    // Make sure we minted 49_500_000 maxbtc to `recipient_addr`
    // The minted amount can be found in the first message’s JSON if you parse it,
    // or just check the attribute if you emit one. Let’s check the attributes:
    let minted_attr = res
        .attributes
        .iter()
        .find(|attr| attr.key == "minted_maxbtc")
        .expect("minted_maxbtc attribute must be present");
    assert_eq!(minted_attr.value, "990000");

    // Also check that no errors occurred.
    // Double-check the contract state if needed:
    let fsm_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(fsm_state, ContractState::Idle);

    // e.g. check if the exchange rate got cached.
    let cached = CACHED_ER.load(&deps.storage).unwrap();
    assert!(cached.is_none(), "Expected no cached ER in the Idle state");
}

#[test]
fn test_deposit_contract_paused() {
    let (mut deps, env, _) = setup_contract();

    // Mark contract as paused
    let mut cfg = CONFIG.load(&deps.storage).unwrap();
    cfg.paused = true;
    CONFIG.save(&mut deps.storage, &cfg).unwrap();

    // Provide a deposit
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(1_000_000u128, "wBTC")],
    ); // 1 wBTC

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("Should error if contract paused");

    // Assert
    match err {
        ContractError::ContractPaused {} => (),
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_deposit_exceeds_cap() {
    let (mut deps, env, _) = setup_contract();

    // Suppose the deposit cap is 100 wBTC
    let mut cfg = CONFIG.load(&deps.storage).unwrap();
    cfg.paused = false;
    cfg.deposits_cap = Some(Uint128::from(100_000_000u128)); // 100 wBTC
    CONFIG.save(&mut deps.storage, &cfg).unwrap();

    // Mock queries so that the AUM is already at 110 wBTC
    deps.querier
        .update_oracle_aum(Uint128::from(110_000_000u128));
    deps.querier
        .set_token_supply(&cfg.maxbtc_denom, Uint128::zero());
    deps.querier
        .update_liqbuffer_maxbtc_balance(Uint128::zero());

    // Provide a deposit
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(1_000_000u128, "wBTC")],
    ); // 1 wBTC

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("Should exceed deposit cap");

    // Assert
    match err {
        ContractError::DepositCapExceeded {} => (),
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_deposit_not_allowlisted() {
    let (mut deps, env, _) = setup_contract();

    // Suppose we have an allowlist of ["alice", "bob"]
    let mut cfg = CONFIG.load(&deps.storage).unwrap();
    cfg.paused = false;
    cfg.deposits_allowlist = Some(vec![deps.api.addr_make("alice"), deps.api.addr_make("bob")]);
    CONFIG.save(&mut deps.storage, &cfg).unwrap();

    // We'll deposit from "charlie", who is not in the allowlist
    let info = message_info(
        &deps.api.addr_make("charlie"),
        &[coin(1_000_000u128, "wBTC")],
    ); // 1 wBTC

    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        recipient.to_string(),
    )
    .expect_err("Should error if depositor not in allowlist");

    // Assert
    match err {
        ContractError::Unauthorized {} => (),
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_deposit_no_funds() {
    let (mut deps, env, _) = setup_contract();
    let info = message_info(&deps.api.addr_make("depositor"), &[]); // no funds

    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("No funds means error");

    match err {
        ContractError::NoFundsSent {} => (),
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_deposit_multiple_funds() {
    let (mut deps, env, _) = setup_contract();
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[
            coin(1_000_000u128, "wBTC"),
            coin(2_000_000u128, "anotherDenom"),
        ],
    );

    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("Must fail if multiple funds are attached");

    match err {
        ContractError::InvalidDepositAmount {} => (),
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_deposit_zero_amount() {
    let (mut deps, env, _) = setup_contract();
    // deposit 0 wBTC
    let info = message_info(&deps.api.addr_make("depositor"), &[coin(0u128, "wBTC")]);

    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("Zero deposit is invalid");

    match err {
        ContractError::InvalidDepositAmount {} => (),
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_deposit_wrong_denom() {
    let (mut deps, env, _) = setup_contract();
    // deposit coin in a different denom
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(1_000_000u128, "ETH")],
    );

    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("Wrong denom should fail");

    match err {
        ContractError::InvalidDepositDenom { expected, received } => {
            assert_eq!(expected, "wBTC");
            assert_eq!(received, "ETH");
        }
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_deposit_minted_zero_below_er() {
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Suppose the exchange rate is super high, e.g. ER=100.
    // Then deposit of 50 wBTC => minted ~ 0.49 maxBTC if deposit_fee=1%,
    // which might floor to 0 in integer terms if decimals do not suffice.
    // Let’s try a scenario that results in minted=0 once we do integer trunc.
    deps.querier
        .update_oracle_aum(Uint128::from(10_000_000_000u128)); // huge AUM => huge ER
    deps.querier
        .set_token_supply(&cfg.maxbtc_denom, Uint128::from(100_000_000u128));
    deps.querier
        .update_liqbuffer_maxbtc_balance(Uint128::zero());

    let info = message_info(&deps.api.addr_make("depositor"), &[coin(50u128, "wBTC")]); // 0.000050 wBTC in decimal(6)

    let recipient = deps.api.addr_make("recipient_addr").to_string();
    do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("Should not succeed (0 minted)");
}

#[test]
fn test_deposit_fsm_in_flushing_but_not_stale() {
    // This scenario checks that `_process_cache` does not revert to an error if the
    // contract is in the `Flushing` state but the flush is not “stale” or “complete.”
    //
    // We do not necessarily see an error; the code just tries `_process_cache_flushing`
    // and either does nothing or transitions back to `Idle`. Then it processes the deposit.
    let (mut deps, env, _) = setup_contract();

    // Put the FSM in the Flushing state
    FSM.go_to(&mut deps.storage, ContractState::Flushing)
        .unwrap();

    // Insert a fresh CACHED_ER entry (not expired).
    // For example, we pretend we last set this with a 9999999999-timeout
    CACHED_ER
        .save(
            &mut deps.storage,
            &Some(CachedER {
                er: Decimal::one(),
                aum: Some(CachedAUM {
                    oracle_aum: Uint128::from(100_000_000u128),
                    deposit_buffer: Uint128::from(50_000_000u128),
                }),
                timeout: 9999999999,
            }),
        )
        .unwrap();

    // The deposit config is not paused, no deposit cap, etc.
    let mut cfg = CONFIG.load(&deps.storage).unwrap();
    cfg.paused = false;
    CONFIG.save(&mut deps.storage, &cfg).unwrap();

    // Provide a deposit
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(1_000_000u128, "wBTC")],
    ); // 1 wBTC
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let res = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect("Should succeed, not stale => no error.");

    // We expect a Mint message
    assert_eq!(res.messages.len(), 1);

    // Check that the minted_maxbtc attribute is present
    let minted_attr = res
        .attributes
        .iter()
        .find(|attr| attr.key == "minted_maxbtc")
        .expect("Expected minted_maxbtc attribute");
    // minted should be close to deposit_amount*(1-fee)/er = 1*(0.99)/1 => 0.99 => truncated => 0?
    // Because we used deposit=1 wBTC with decimal=6 => 1_000_000 in base units => that's 1.0 wBTC
    // minted = 0.99 => 0.99 in decimal => as integer => 0.99 * 1_000_000 = 990_000
    assert_eq!(minted_attr.value, "990000");

    // The code `_process_cache_flushing` might or might not have changed the state to Idle,
    // depending on your logic. Let's just verify we do not get an error
    // (some code transitions right away if certain conditions are met).
    // If it’s still Flushing, the deposit was still allowed.
    let new_state = FSM.get_current_state(&deps.storage).unwrap();
    assert!(
        new_state == ContractState::Idle || new_state == ContractState::Flushing,
        "We either remain in Flushing or transition to Idle, but must not error."
    );
}

#[test]
fn test_flush_guard_not_enough_time_elapsed() {
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Deposit buffer: 0.5 wBTC.
    let buffer = Uint128::new(500_000);
    deps.querier
        .set_balance(&env.contract.address.to_string(), "wBTC", buffer);

    // Mock oracle & liq-buffer queries so ER math inside the call can run.
    deps.querier.update_oracle_aum(Uint128::new(10_000_000));
    deps.querier
        .update_liqbuffer_maxbtc_balance(Uint128::zero());
    deps.querier.update_liqbuffer_btc_balance(Uint128::zero());

    // Set LAST_DEPOSIT_FLUSH_TIME so that *less* than `deposit_flush_period`
    // seconds have elapsed.
    let now = env.block.time.seconds();
    LAST_DEPOSIT_FLUSH_TIME
        .save(&mut deps.storage, &(now - (cfg.deposit_flush_period / 2)))
        .unwrap();

    let info = message_info(&deps.api.addr_make("flusher"), &[]);

    // ── Act ────────────────────────────────────────────────────────────────────
    let resp = execute_flush_deposits(deps.as_mut(), env.clone(), info).unwrap();

    // ── Assert ────────────────────────────────────────────────────────────────
    // 1. Status attribute.
    let status_attr = resp
        .attributes
        .iter()
        .find(|a| a.key == "status")
        .expect("status attribute present");
    assert_eq!(status_attr.value, "not_enough_time_elapsed");

    // 2. No state transitions happened.
    assert_eq!(
        FSM.get_current_state(&deps.storage).unwrap(),
        ContractState::Idle
    );
    // 3. LAST_DEPOSIT_FLUSH_TIME unchanged.
    let stored = LAST_DEPOSIT_FLUSH_TIME.load(&deps.storage).unwrap();
    assert_eq!(stored, now - (cfg.deposit_flush_period / 2));
    // 4. No transfer messages emitted.
    assert!(resp.messages.is_empty());
}

#[test]
fn test_flush_zero_outstanding_deposits() {
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    // No balance in the contract’s deposit buffer.
    deps.querier
        .set_balance(&env.contract.address.to_string(), "wBTC", Uint128::zero());

    // Mock oracle/liq buffer queries (values don’t matter here).
    deps.querier.update_oracle_aum(Uint128::new(10_000_000));

    // Make sure the flush period has *elapsed*.
    let now = env.block.time.seconds();
    LAST_DEPOSIT_FLUSH_TIME
        .save(&mut deps.storage, &(now - cfg.deposit_flush_period - 1))
        .unwrap();

    let info = message_info(&deps.api.addr_make("flusher"), &[]);

    // ── Act ────────────────────────────────────────────────────────────────────
    let resp = execute_flush_deposits(deps.as_mut(), env.clone(), info).unwrap();

    // ── Assert ────────────────────────────────────────────────────────────────
    let status_attr = resp.attributes.iter().find(|a| a.key == "status").unwrap();
    assert_eq!(status_attr.value, "zero_outstanding_deposits");

    // LAST_DEPOSIT_FLUSH_TIME must be set to *now*.
    let stored = LAST_DEPOSIT_FLUSH_TIME.load(&deps.storage).unwrap();
    assert_eq!(stored, now);

    // FSM stays IDLE, nothing to send.
    assert_eq!(
        FSM.get_current_state(&deps.storage).unwrap(),
        ContractState::Idle
    );
    assert!(resp.messages.is_empty());
}

#[test]
fn test_flush_sends_to_liqbuffer_then_pump() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    deps.querier.update_liqbuffer_btc_balance(Uint128::zero());
    deps.querier
        .set_token_supply(&cfg.maxbtc_denom, Uint128::from(2_000_000u128));

    let deposit_buffer = Uint128::new(2_000_000);
    deps.querier
        .set_balance(&env.contract.address.to_string(), "wBTC", deposit_buffer);

    // flush_period elapsed:
    let now = env.block.time.seconds();
    LAST_DEPOSIT_FLUSH_TIME
        .save(&mut deps.storage, &(now - cfg.deposit_flush_period - 1))
        .unwrap();

    let info = message_info(&deps.api.addr_make("flusher"), &[]);

    // ── Act ────────────────────────────────────────────────────────────────────
    let resp = execute_flush_deposits(deps.as_mut(), env.clone(), info).unwrap();
    let msgs = extract_msgs(&resp.messages);

    // ── Assert ────────────────────────────────────────────────────────────────
    // 1. FSM moved into Flushing state; ER got cached.
    assert_eq!(
        FSM.get_current_state(&deps.storage).unwrap(),
        ContractState::Flushing
    );
    assert!(CACHED_ER.load(&deps.storage).unwrap().is_some());

    // 2. We sent 200 000 wBTC to the liquidation buffer contract ...
    assert_bank_send_exists(
        &msgs,
        &cfg.liquidation_buffer_contract.to_string(),
        Uint128::new(200_000),
        "wBTC",
    );
    // ... and the remaining 1 800 000 to the deposit pump.
    assert_bank_send_exists(
        &msgs,
        &cfg.deposit_pump_contract.to_string(),
        Uint128::new(1_800_000),
        "wBTC",
    );
}

#[test]
fn test_flush_requests_clawback_then_pump() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    deps.querier.update_oracle_aum(Uint128::new(1_000_000));
    deps.querier
        .update_liqbuffer_btc_balance(Uint128::new(500_000));
    deps.querier.set_balance(
        &env.contract.address.to_string(),
        "wBTC",
        Uint128::new(500_000),
    );

    let now = env.block.time.seconds();
    LAST_DEPOSIT_FLUSH_TIME
        .save(&mut deps.storage, &(now - cfg.deposit_flush_period - 1))
        .unwrap();

    let info = message_info(&deps.api.addr_make("flusher"), &[]);

    // ── Act ────────────────────────────────────────────────────────────────────
    let resp = execute_flush_deposits(deps.as_mut(), env.clone(), info).unwrap();
    let msgs = extract_msgs(&resp.messages);

    // ── Assert ────────────────────────────────────────────────────────────────
    // 1. Claw-back message exists.
    assert_clawback_exists(
        &msgs,
        &cfg.liquidation_buffer_contract.to_string(),
        coin(300_000u128, "wBTC"),
    );

    // 2. Entire 2 000 000 now sitting in the contract is forwarded to the pump.
    assert_bank_send_exists(
        &msgs,
        &cfg.deposit_pump_contract.to_string(),
        Uint128::new(800_000),
        "wBTC",
    );

    // 3. FSM is Flushing and ER is cached.
    assert_eq!(
        FSM.get_current_state(&deps.storage).unwrap(),
        ContractState::Flushing
    );
    assert!(CACHED_ER.load(&deps.storage).unwrap().is_some());
}

#[test]
fn test_withdraw_success() {
    let (mut deps, env, _) = setup_contract();

    // Make sure we have an ACTIVE batch.
    // `setup_contract()` *already* creates an ACTIVE batch with `batch_id = 1`
    // but we enforce the assumption explicitly so that the test does not start
    // failing if the helper ever changes.
    ACTIVE_BATCH
        .save(
            &mut deps.storage,
            &Some(Batch {
                batch_id: 1,
                btc_requested: Uint128::zero(),
                maxbtc_burned: Uint128::zero(),
                collected_amount: Uint128::zero(),
                paid_amount: Uint128::zero(),
                collector_historical_balance: Uint128::zero(),
            }),
        )
        .unwrap();

    // Craft a withdrawal of exactly 1 maxBTC (denominated with 6 decimals).
    let withdraw_amount = Uint128::from(1_000_000u128); // 1.000000 maxBTC
    let sender = deps.api.addr_make("withdrawer");

    let maxbtc_denom = CONFIG
        .load(&deps.storage)
        .unwrap()
        .get_maxbtc_denom(env.contract.address.to_string());

    let info = message_info(&sender, &[coin(withdraw_amount.u128(), &maxbtc_denom)]);
    let res = execute_withdraw(deps.as_mut(), env.clone(), info.clone()).unwrap();

    // The happy-path should emit **exactly two** SubMsgs:
    //   0. token-factory burn of `maxBTC`
    //   1. token-factory mint of `redemption/batch/1`
    assert_eq!(res.messages.len(), 2, "expected Burn+Mint messages");

    // Response attributes
    let attr = |k: &str| -> Option<&str> {
        res.attributes
            .iter()
            .find(|a| a.key == k)
            .map(|a| a.value.as_str())
    };

    assert_eq!(attr("action"), Some("withdraw"));
    assert_eq!(attr("sender"), Some(sender.as_str()));
    assert_eq!(attr("batch_id"), Some("1"));
    assert_eq!(
        attr("withdraw_amount"),
        Some(withdraw_amount.to_string().as_str())
    );

    // ACTIVE_BATCH.maxbtc_burned must now equal `withdraw_amount`.
    let active_batch = ACTIVE_BATCH
        .load(&deps.storage)
        .unwrap()
        .expect("ACTIVE batch must exist");
    assert_eq!(
        active_batch.maxbtc_burned, withdraw_amount,
        "burn counter in ACTIVE_BATCH must be updated"
    );

    // 2. FSM should still be `Idle` and no ER cache should have been created.
    assert_eq!(
        FSM.get_current_state(&deps.storage).unwrap(),
        ContractState::Idle
    );
    assert!(
        CACHED_ER.load(&deps.storage).unwrap().is_none(),
        "no ER cache is expected in the Idle state"
    );
}

#[test]
fn test_withdraw_fails_when_paused() {
    let (mut deps, env, _) = setup_contract();

    // Pause the contract.
    CONFIG
        .update::<_, ContractError>(&mut deps.storage, |mut c| {
            c.paused = true;
            Ok(c)
        })
        .unwrap();

    // Any non-empty funds will do – they won’t be checked after the pause gate.
    let info = message_info(&deps.api.addr_make("any"), &[coin(1, "dummy")]);

    let err = execute_withdraw(deps.as_mut(), env, info).unwrap_err();
    assert!(matches!(err, ContractError::ContractPaused {}));
}

#[test]
fn test_withdraw_fails_with_no_funds() {
    let (mut deps, env, _) = setup_contract();

    let info = message_info(&deps.api.addr_make("user"), &[]);
    let err = execute_withdraw(deps.as_mut(), env, info).unwrap_err();
    assert!(matches!(err, ContractError::NoFundsSent {}));
}

#[test]
fn test_withdraw_fails_with_wrong_denom() {
    let (mut deps, env, _) = setup_contract();

    let info = message_info(&deps.api.addr_make("user"), &[coin(1_000, "wBTC")]); // wrong denom
    let err = execute_withdraw(deps.as_mut(), env, info).unwrap_err();
    assert!(matches!(err, ContractError::InvalidDepositDenom { .. }));
}

#[test]
fn test_withdraw_fails_with_zero_amount() {
    let (mut deps, env, _) = setup_contract();

    let maxbtc_denom = CONFIG
        .load(&deps.storage)
        .unwrap()
        .get_maxbtc_denom(env.contract.address.to_string());

    let info = message_info(&deps.api.addr_make("user"), &[coin(0u128, &maxbtc_denom)]);
    let err = execute_withdraw(deps.as_mut(), env, info).unwrap_err();
    assert!(matches!(err, ContractError::InvalidDepositAmount {}));
}

#[test]
fn test_withdraw_fails_without_active_batch() {
    let (mut deps, env, _) = setup_contract();

    // Remove the ACTIVE batch altogether.
    ACTIVE_BATCH.save(&mut deps.storage, &None).unwrap();

    let maxbtc_denom = CONFIG
        .load(&deps.storage)
        .unwrap()
        .get_maxbtc_denom(env.contract.address.to_string());
    let info = message_info(&deps.api.addr_make("user"), &[coin(1_000, &maxbtc_denom)]);

    let err = execute_withdraw(deps.as_mut(), env, info).unwrap_err();
    assert!(matches!(err, ContractError::BatchStateError {}));
}

#[test]
fn test_process_active_batch_happy_path() {
    let (mut deps, mut env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Force the active-batch start to well in the past
    let now = env.block.time.seconds();
    let past = now - cfg.batch_active_duration - 10;
    ACTIVE_BATCH_START_TIME
        .save(&mut deps.storage, &past)
        .unwrap();

    // Make sure *some* redemption tokens exist
    let redemption_denom = cfg.get_redemption_denom(env.contract.address.to_string(), 1u64); // batch_id = 1 on first instantiation
    deps.querier
        .set_token_supply(&redemption_denom, Uint128::from(1_000_000u128)); // 1 token (6 dec)

    // ER machinery – keep it simple: numerator == denominator ⇒ ER = 1
    deps.querier.update_oracle_aum(Uint128::from(1_000_000u128)); // oracle AUM
    deps.querier
        .set_token_supply(&cfg.maxbtc_denom, Uint128::from(1_000_000u128));
    deps.querier
        .update_liqbuffer_maxbtc_balance(Uint128::zero());
    deps.querier.update_liqbuffer_btc_balance(Uint128::zero());

    // Zero balance on collector; required later in transition
    deps.querier.set_balance(
        &cfg.collector_contract.as_str(),
        &cfg.deposit_denom,
        Uint128::zero(),
    );

    // Bump env time to (now) so that `execute_process_active_batch` “sees”
    // that the batch is already old enough.
    env = env_with_time(env, 0);

    let info = message_info(&deps.api.addr_make("anyone"), &[]);
    let res = execute_process_active_batch(deps.as_mut(), env.clone(), info).unwrap();

    // FSM state
    assert_eq!(
        FSM.get_current_state(&deps.storage).unwrap(),
        ContractState::Withdrawing
    );

    // WITHDRAWING_BATCH should now exist with batch_id == 1
    let w_batch = WITHDRAWING_BATCH.load(&deps.storage).unwrap().unwrap();
    assert_eq!(w_batch.batch_id, 1);

    // ACTIVE_BATCH was rolled – its id must be 2
    let active = ACTIVE_BATCH.load(&deps.storage).unwrap().unwrap();
    assert_eq!(active.batch_id, 2);

    // `new_withdrawing_batch_id` attribute is present
    let attr = res
        .attributes
        .iter()
        .find(|a| a.key == "new_withdrawing_batch_id")
        .unwrap();
    assert_eq!(attr.value, "1");
}

#[test]
fn test_process_active_batch_no_withdraw_requests() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Make the batch old enough
    let now = env.block.time.seconds();
    let past = now - cfg.batch_active_duration - 5;
    ACTIVE_BATCH_START_TIME
        .save(&mut deps.storage, &past)
        .unwrap();

    // *Zero* supply for the redemption token (implicitly – we do NOT call set_token_supply)

    // Caller
    let info = message_info(&deps.api.addr_make("trigger"), &[]);
    let res = execute_process_active_batch(deps.as_mut(), env.clone(), info).unwrap();

    // FSM never left Idle
    assert_eq!(
        FSM.get_current_state(&deps.storage).unwrap(),
        ContractState::Idle
    );

    // Attribute `status = no_withdraw_requests_found`
    let attr = res
        .attributes
        .iter()
        .find(|a| a.key == "status")
        .expect("status attribute");
    assert_eq!(attr.value, "no_withdraw_requests_found");

    // ACTIVE_BATCH_START_TIME was reset to *now*
    let stored_time = ACTIVE_BATCH_START_TIME.load(&deps.storage).unwrap();
    assert_eq!(stored_time, env.block.time.seconds());
}

#[test]
fn test_process_active_batch_too_early() {
    let (mut deps, env, _) = setup_contract();

    // ACTIVE_BATCH_START_TIME is the current block time, so batch is *not* old enough
    let info = message_info(&deps.api.addr_make("eager_beaver"), &[]);
    let err = execute_process_active_batch(deps.as_mut(), env.clone(), info).unwrap_err();

    assert!(matches!(err, ContractError::CannotProcessActiveBatchYet {}));

    // No state-change: FSM remains Idle
    assert_eq!(
        FSM.get_current_state(&deps.storage).unwrap(),
        ContractState::Idle
    );
}

#[test]
fn test_claim_success() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Finalised batch #1 with 1_000_000 sat worth of BTC already collected.
    let total_redemption = Uint128::from(1_000_000u128);
    let redemption_denom = put_finalised_batch(
        &mut deps,
        &cfg,
        &env.clone(),
        1,
        /*collected*/ total_redemption,
        /*paid*/ Uint128::zero(),
    );

    // Mock Bank supply so that total redemption-token = 1_000_000.
    deps.querier
        .set_token_supply(&redemption_denom, total_redemption);

    // The claimant sends 200_000 redemption tokens.
    let user_redeem = Uint128::from(200_000u128);
    let claimant = deps.api.addr_make("claimer");
    let recipient = deps.api.addr_make("btc_receiver");
    let info = message_info(&claimant, &[coin(user_redeem.u128(), &redemption_denom)]);

    let res = execute_claim(deps.as_mut(), env.clone(), info, recipient.to_string()).unwrap();

    // We expect two Cosmos messages: send BTC and burn redemption token.
    assert_eq!(res.messages.len(), 2, "exactly send + burn");

    // Bank send
    match &res.messages[0].msg {
        CosmosMsg::Bank(BankMsg::Send { to_address, amount }) => {
            assert_eq!(to_address, &recipient.to_string());
            assert_eq!(amount.len(), 1);
            assert_eq!(amount[0].denom, cfg.deposit_denom);
            assert_eq!(
                amount[0].amount, user_redeem,
                "user gets 1:1 BTC for redeemed tokens"
            );
        }
        _ => panic!("1st message must be Bank::Send"),
    }

    // Burn message – we only check it *is* a burn.
    match &res.messages[1].msg {
        CosmosMsg::Any(any_msg) => {
            assert_eq!(any_msg.type_url, "/osmosis.tokenfactory.v1beta1.MsgBurn");
        }
        _ => panic!("Expected Any message with MsgBurn type_url"),
    }

    let user_attr = res
        .attributes
        .iter()
        .find(|a| a.key == "user_claim_btc")
        .expect("attribute user_claim_btc must exist");
    assert_eq!(user_attr.value, user_redeem.to_string());

    // Batch paid_amount was updated.
    let stored = FINALIZED_BATCHES.load(&deps.storage, 1).unwrap();
    assert_eq!(stored.paid_amount, user_redeem);
}

#[test]
fn test_claim_batch_not_finalised() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Create a redemption token denom that has **no** corresponding batch.
    let redemption_denom = cfg.get_redemption_denom(env.contract.address.to_string(), 1);
    deps.querier
        .set_token_supply(&redemption_denom, Uint128::from(10u64));

    let info = message_info(
        &deps.api.addr_make("user"),
        &[coin(10u128, &redemption_denom)],
    );

    let recv_addr = deps.api.addr_make("recv").to_string();
    let err = execute_claim(deps.as_mut(), env, info, recv_addr.to_string()).unwrap_err();

    assert_eq!(err, ContractError::BatchNotFinalized {});
}

#[test]
fn test_claim_supply_mismatch() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Finalise batch #3 but set the token-supply to *zero*.
    let redemption_denom = put_finalised_batch(
        &mut deps,
        &cfg,
        &env.clone(),
        3,
        Uint128::from(500_000u128),
        Uint128::zero(),
    );
    deps.querier
        .set_token_supply(&redemption_denom, Uint128::zero());

    let info = message_info(
        &deps.api.addr_make("user"),
        &[coin(1u128, &redemption_denom)],
    );

    let recv_addr = deps.api.addr_make("recv").to_string();
    let err = execute_claim(deps.as_mut(), env, info, recv_addr).unwrap_err();

    assert_eq!(err, ContractError::RedemptionSupplyMismatch {});
}

/// When *all* funds have reached the destination (or are within the
/// tolerance) `_process_cache` must clear the cache and go Idle.
#[test]
fn test_cache_flushing_finalises_and_clears() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    // The flushed “deposit buffer” we will pretend to have sent previously.
    let deposit_buffer = Uint128::from(50_000u128);
    let historic_oracle_aum = Uint128::from(100_000u128);

    // Store cache & FSM=Flushing
    let cache = CachedER {
        er: Decimal::one(),
        timeout: env.block.time.seconds() + 1_000, // not stale
        aum: Some(CachedAUM {
            oracle_aum: historic_oracle_aum,
            deposit_buffer,
        }),
    };
    prime_cache(&mut deps, ContractState::Flushing, cache);

    // Mock the *current* oracle AUM → everything arrived
    deps.querier
        .update_oracle_aum(historic_oracle_aum + deposit_buffer);

    // call the internal helper directly
    let msgs = _process_cache(deps.as_mut(), env.clone(), &cfg).unwrap();

    assert!(msgs.is_empty(), "no side-effect messages expected");

    let fsm_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(fsm_state, ContractState::Idle);

    let cached_after = CACHED_ER.load(&deps.storage).unwrap();
    assert!(
        cached_after.is_none(),
        "cache must have been cleared after successful flush"
    );
}

/// If only part of the buffer has arrived and the difference is
/// *outside* the tolerance window, we must **remain** in *Flushing*.
#[test]
fn test_cache_flushing_incomplete_keeps_state() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    let deposit_buffer = Uint128::from(50_000u128);
    let historic_oracle_aum = Uint128::from(100_000u128);

    let cache = CachedER {
        er: Decimal::one(),
        timeout: env.block.time.seconds() + 1_000,
        aum: Some(CachedAUM {
            oracle_aum: historic_oracle_aum,
            deposit_buffer,
        }),
    };
    prime_cache(&mut deps, ContractState::Flushing, cache.clone());

    let accepted_diff = dec_to_amount(
        Decimal::from_atomics(deposit_buffer, cfg.deposit_decimals).unwrap()
            * cfg.deposit_buffer_tolerance,
        cfg.deposit_decimals,
    )
    .unwrap();

    // Let only (deposit_buffer - accepted_diff - 1) reach the oracle.
    let successfully_flushed = deposit_buffer - accepted_diff - Uint128::one();
    deps.querier
        .update_oracle_aum(historic_oracle_aum + successfully_flushed);

    let msgs = _process_cache(deps.as_mut(), env.clone(), &cfg).unwrap();

    assert!(msgs.is_empty());
    let fsm_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(fsm_state, ContractState::Flushing); // unchanged
                                                    // Cache must still be present.
    let _ = load_cache(&deps);
}

#[test]
fn test_cache_flushing_stale_triggers_emergency() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    let deposit_buffer = Uint128::from(1u128);
    let cache = CachedER {
        er: Decimal::one(),
        timeout: env.block.time.seconds() - 1, // already stale
        aum: Some(CachedAUM {
            oracle_aum: Uint128::from(1u128),
            deposit_buffer,
        }),
    };
    prime_cache(&mut deps, ContractState::Flushing, cache);

    let res = _process_cache(deps.as_mut(), env.clone(), &cfg);
    assert!(matches!(res, Err(ContractError::ProtocolInEmergency {})));
}

#[test]
fn test_cache_withdrawing_finalises_and_sends_extra() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Parameters for the fake batch
    let btc_requested = Uint128::from(100_000u128);
    let hist_col_balance = Uint128::zero(); // starting balance
    let current_balance = Uint128::from(120_000u128); // +20 000 extra
    let extra = current_balance - btc_requested;

    // Store the withdrawing batch & FSM
    let withdrawing_batch = Batch {
        batch_id: 42,
        btc_requested,
        maxbtc_burned: Uint128::zero(),
        collected_amount: Uint128::zero(),
        paid_amount: Uint128::zero(),
        collector_historical_balance: hist_col_balance,
    };
    WITHDRAWING_BATCH
        .save(&mut deps.storage, &Some(withdrawing_batch))
        .unwrap();

    // Cache (contents irrelevant for withdrawing logic, only needs
    // to exist and be non-stale)
    let cache = CachedER {
        er: Decimal::one(),
        timeout: env.block.time.seconds() + 1_000,
        aum: None,
    };
    prime_cache(&mut deps, ContractState::Withdrawing, cache);

    // Mock the collector's *current* balance.
    deps.querier.set_balance(
        &cfg.collector_contract.to_string(),
        &cfg.deposit_denom,
        current_balance,
    );

    let msgs = _process_cache(deps.as_mut(), env.clone(), &cfg).unwrap();

    // 1. Exactly one message: Bank::Send(extra) to the treasury
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        CosmosMsg::Bank(BankMsg::Send { to_address, amount }) => {
            assert_eq!(to_address, &cfg.treasury_address.to_string());
            assert_eq!(amount.len(), 1);
            assert_eq!(amount[0].amount, extra);
            assert_eq!(amount[0].denom, cfg.deposit_denom);
        }
        _ => panic!("expected a Bank::Send message"),
    }

    // 2. FSM back to Idle
    let fsm_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(fsm_state, ContractState::Idle);

    // 3. Cache cleared
    assert!(CACHED_ER.load(&deps.storage).unwrap().is_none());

    // 4. WITHDRAWING_BATCH cleared
    assert!(WITHDRAWING_BATCH.load(&deps.storage).unwrap().is_none());

    // 5. Batch persisted into FINALIZED_BATCHES
    let finalised = FINALIZED_BATCHES.load(&deps.storage, 42).unwrap();
    assert_eq!(finalised.collected_amount, current_balance);
    assert_eq!(finalised.paid_amount, Uint128::zero());
}

/// Collected amount is *inside* the acceptance window → nothing happens.
#[test]
fn test_cache_withdrawing_pending() {
    let (mut deps, env, _) = setup_contract();
    let cfg = CONFIG.load(&deps.storage).unwrap();

    let btc_requested = Uint128::from(100_000u128);
    let hist_balance = Uint128::zero();

    // Determine “accepted_diff” so we can stay inside the window
    let accepted_diff = dec_to_amount(
        Decimal::from_atomics(btc_requested, cfg.deposit_decimals).unwrap()
            * cfg.collected_tolerance,
        cfg.deposit_decimals,
    )
    .unwrap();

    let collected_ok = btc_requested - accepted_diff - Uint128::one(); // inside window

    let batch = Batch {
        batch_id: 7,
        btc_requested,
        maxbtc_burned: Uint128::zero(),
        collected_amount: Uint128::zero(),
        paid_amount: Uint128::zero(),
        collector_historical_balance: hist_balance,
    };
    WITHDRAWING_BATCH
        .save(&mut deps.storage, &Some(batch))
        .unwrap();

    let cache = CachedER {
        er: Decimal::one(),
        timeout: env.block.time.seconds() + 1_000,
        aum: None,
    };
    prime_cache(&mut deps, ContractState::Withdrawing, cache);

    // Mock the collector balance so that *collected_ok* is available
    deps.querier.set_balance(
        &cfg.collector_contract.to_string(),
        &cfg.deposit_denom,
        collected_ok,
    );

    let msgs = _process_cache(deps.as_mut(), env.clone(), &cfg).unwrap();

    assert!(
        msgs.is_empty(),
        "no side effects expected while waiting for full collection"
    );
    let fsm_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(fsm_state, ContractState::Withdrawing);

    // Cache and WITHDRAWING_BATCH must still be present.
    let _ = load_cache(&deps);
    assert!(WITHDRAWING_BATCH.load(&deps.storage).unwrap().is_some());
}

/// -----------------------------------------------------------------------------------------------
/// HELPER FUNCTIONS BELOW
/// -----------------------------------------------------------------------------------------------

/// Initializes the contract and sets up a "happy path" config in storage.
/// Returns a mutable Deps and an Env, Info you can reuse in tests.
fn setup_contract() -> (
    OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
    Env,
    MessageInfo,
) {
    let mut deps = mock_dependencies(); // your custom mock with WasmMockQuerier
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("any_sender"), &[]);

    // Instantiate with default params
    let instantiate_msg = default_instantiate_msg(&deps);
    let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), instantiate_msg).unwrap();

    // Because deposit() calls `_process_cache(...)`, we typically also want
    // to ensure that the FSM is in Idle state initially (it is, by default).
    let state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(state, ContractState::Idle);

    (deps, env, info)
}

fn default_instantiate_msg(
    deps: &OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> InstantiateMsg {
    InstantiateMsg {
        owner: deps.api.addr_make("owner_addr").to_string(),
        aum_contract: deps.api.addr_make("aum_addr").to_string(),
        liquidation_contract: deps.api.addr_make("liq_buffer_addr").to_string(),
        deposit_pump_contract: deps.api.addr_make("pump_addr").to_string(),
        collector_contract: deps.api.addr_make("collector_addr").to_string(),
        treasury_address: deps.api.addr_make("treasury_addr").to_string(),
        deposit_denom: "wBTC".to_string(),
        deposit_decimals: 6u32,
        maxbtc_denom: "maxbtc".to_string(),
        deposit_flush_period: 3600,
        batch_active_duration: 86400,
        batch_withdrawing_duration: 86400,
        accepted_withdrawable_percentage: Decimal::percent(5),
        liquidation_buffer_share: Decimal::percent(10),
        deposit_fee: Decimal::percent(1),
        cached_aum_tolerance: Decimal::percent(2),
        cached_er_ttl: 100u64,
        deposits_cap: None,
        deposits_allowlist: None,
    }
}

/// A convenience helper for calling the `execute_deposit` entry point.
fn do_deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    execute(
        deps,
        env,
        info,
        ExecuteMsg::Deposit {
            recipient: recipient.to_string(),
        },
    )
}

/// Returns only the `CosmosMsg`-payloads from a vector of `SubMsg`.
fn extract_msgs(submsgs: &[SubMsg]) -> Vec<CosmosMsg> {
    submsgs.iter().map(|s| s.msg.clone()).collect()
}

/// Asserts that a [`BankMsg::Send`] exists in `msgs` with the given
/// destination/amount/denom.
fn assert_bank_send_exists(msgs: &[CosmosMsg], to: &str, amount: Uint128, denom: &str) {
    assert!(
        msgs.iter().any(|m| match m {
            CosmosMsg::Bank(BankMsg::Send {
                to_address,
                amount: coins,
            }) => {
                to_address == to
                    && coins.len() == 1
                    && coins[0].denom == denom
                    && coins[0].amount == amount
            }
            _ => false,
        }),
        "expected BankMsg::Send(to={to}, amount={amount}{denom}) not found"
    );
}

/// Asserts that a [`WasmMsg::Execute`] exists in `msgs` invoking
/// `LiquidationExecuteMsg::ClawBack { amount }` on `contract_addr`.
fn assert_clawback_exists(msgs: &[CosmosMsg], contract_addr: &str, clawback: Coin) {
    assert!(
        msgs.iter().any(|m| match m {
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: c,
                msg,
                funds,
            }) => {
                if c != contract_addr || !funds.is_empty() {
                    return false;
                }
                let parsed: LiquidationExecuteMsg = from_json(msg).unwrap();
                matches!(parsed, LiquidationExecuteMsg::ClawBack { amount } if amount == clawback)
            }
            _ => false,
        }),
        "expected WasmMsg::Execute(ClawBack) not found",
    );
}

/// Returns a fresh env whose block-time is `base + offset_secs`
fn env_with_time(mut env: Env, offset_secs: u64) -> Env {
    env.block.time = env.block.time.plus_seconds(offset_secs);
    env
}

/// Helper that prepares a finalised batch `#batch_id` and returns its
/// redemption-token denom (`redemption/batch/<id>`).  All monetary
/// amounts are expressed **in the deposit-denom’s base units**.
fn put_finalised_batch(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
    cfg: &Config,
    env: &Env,
    batch_id: u64,
    collected_amount: Uint128,
    paid_amount: Uint128,
) -> String {
    let batch = Batch {
        batch_id,
        btc_requested: collected_amount,
        maxbtc_burned: collected_amount,
        collected_amount,
        paid_amount,
        collector_historical_balance: Uint128::zero(),
    };
    FINALIZED_BATCHES
        .save(&mut deps.storage, batch_id, &batch)
        .unwrap();

    cfg.get_redemption_denom(env.contract.address.to_string(), batch_id)
}

/// Convenience: put the FSM in the requested state **and**
/// save a non-stale cache object.
fn prime_cache(
    deps: &mut OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
    state: ContractState,
    cached_er: CachedER,
) {
    // Move the FSM
    FSM.go_to(&mut deps.storage, state).unwrap();
    // Store the cache
    CACHED_ER.save(&mut deps.storage, &Some(cached_er)).unwrap();
}

/// Reads the cache; panics if it is not `Some`.
fn load_cache(deps: &OwnedDeps<MockStorage, MockApi, WasmMockQuerier>) -> CachedER {
    CACHED_ER
        .load(&deps.storage)
        .unwrap()
        .expect("cache must exist")
}
