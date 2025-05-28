use crate::contract::{execute, instantiate};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::state::{
    CachedAUM, CachedER, ContractState, ACTIVE_BATCH, ACTIVE_BATCH_START_TIME, BATCH_ID_COUNTER,
    CACHED_ER, CONFIG, FSM, LAST_DEPOSIT_FLUSH_TIME,
};
use crate::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockStorage};
use cosmwasm_std::{
    coin, Attribute, Decimal, DepsMut, Env, MessageInfo, OwnedDeps, Response, Uint128,
};

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

#[test]
fn test_first_deposit_success() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    // Make sure the contract is not paused
    let mut cfg = CONFIG.load(&deps.storage).unwrap();
    cfg.paused = false;
    CONFIG.save(&mut deps.storage, &cfg).unwrap();

    // Mock out the queries so that get_exchange_rate() returns ER=1
    //   - AUM = 100_000_000,
    //   - maxBTC supply = 0, etc.
    deps.querier.update_aum(Uint128::from(0u128));
    deps.querier.update_maxbtc_supply(Uint128::zero());
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
    // In this code, because we have no multi-step states, `_process_cache` might revert it
    // if something was in progress. By default, there's no prior state, so we expect no caching:
    let cached = CACHED_ER.load(&deps.storage).unwrap();
    assert!(cached.is_none(), "Expected no cached ER in the Idle state");
}

#[test]
fn test_deposit_contract_paused() {
    // Arrange
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
    // Arrange
    let (mut deps, env, _) = setup_contract();

    // Suppose the deposit cap is 100 wBTC
    let mut cfg = CONFIG.load(&deps.storage).unwrap();
    cfg.paused = false;
    cfg.deposits_cap = Some(Uint128::from(100_000_000u128)); // 100 wBTC
    CONFIG.save(&mut deps.storage, &cfg).unwrap();

    // Mock queries so that the AUM is already at 110 wBTC
    deps.querier.update_aum(Uint128::from(110_000_000u128));
    deps.querier.update_maxbtc_supply(Uint128::zero());
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
    // Arrange
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

    // Act
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
    // Arrange
    let (mut deps, env, _) = setup_contract();
    let info = message_info(&deps.api.addr_make("depositor"), &[]); // no funds

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("No funds means error");

    // Assert
    match err {
        ContractError::NoFundsSent {} => (),
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_deposit_multiple_funds() {
    // Arrange
    let (mut deps, env, _) = setup_contract();
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[
            coin(1_000_000u128, "wBTC"),
            coin(2_000_000u128, "anotherDenom"),
        ],
    );

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("Must fail if multiple funds are attached");

    // Assert
    match err {
        ContractError::InvalidDepositAmount {} => (),
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_deposit_zero_amount() {
    // Arrange
    let (mut deps, env, _) = setup_contract();
    // deposit 0 wBTC
    let info = message_info(&deps.api.addr_make("depositor"), &[coin(0u128, "wBTC")]);

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("Zero deposit is invalid");

    // Assert
    match err {
        ContractError::InvalidDepositAmount {} => (),
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_deposit_wrong_denom() {
    // Arrange
    let (mut deps, env, _) = setup_contract();
    // deposit coin in a different denom
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(1_000_000u128, "ETH")],
    );

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect_err("Wrong denom should fail");

    // Assert
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
    // Arrange
    let (mut deps, env, _) = setup_contract();

    // Suppose the exchange rate is super high, e.g. ER=100.
    // Then deposit of 50 wBTC => minted ~ 0.49 maxBTC if deposit_fee=1%,
    // which might floor to 0 in integer terms if decimals do not suffice.
    // Let’s try a scenario that results in minted=0 once we do integer trunc.
    deps.querier.update_aum(Uint128::from(10_000_000_000u128)); // huge AUM => huge ER
    deps.querier
        .update_maxbtc_supply(Uint128::from(100_000_000u128));
    deps.querier
        .update_liqbuffer_maxbtc_balance(Uint128::zero());

    let info = message_info(&deps.api.addr_make("depositor"), &[coin(50u128, "wBTC")]); // 0.000050 wBTC in decimal(6)

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let res = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient)
        .expect("Should succeed even if minted=0");

    // Assert
    // We expect a Mint message, but the minted amount is 0
    // The contract does NOT explicitly reject zero minted.
    // So it’s a valid (though strange) scenario.
    assert_eq!(res.messages.len(), 1);
    let minted_attr = res
        .attributes
        .iter()
        .find(|attr| attr.key == "minted_maxbtc")
        .expect("minted_maxbtc attribute must be present");
    assert_eq!(minted_attr.value, "0");

    // Because minted=0, the user effectively gets 0 maxBTC minted.
    // This might or might not be desirable, but the current code does not forbid it.
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
