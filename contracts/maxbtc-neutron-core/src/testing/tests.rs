use crate::contract::{execute, execute_flush_deposits, instantiate};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, FeeMinterParams, InstantiateMsg};
use crate::state::{CONFIG, LAST_DEPOSIT_FLUSH_TIME, TOTAL_DEPOSITED};
use crate::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockStorage};
use cosmwasm_std::{
    coin, Attribute, Binary, Decimal, DepsMut, Env, MessageInfo, OwnedDeps, Response, Uint128,
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
    // We expect 2 message: instantiate2 msg for the fee collector, and create_tokenfactory_create_denom_msg
    assert_eq!(res.messages.len(), 2);
    // Assert: check attributes
    let expected_attributes = vec![
        Attribute::new("action", "instantiate"),
        Attribute::new("owner", msg.owner.clone()),
        Attribute::new("allowlist_contract", msg.allowlist_contract.clone()),
        Attribute::new("treasury_address", msg.treasury_address.clone()),
        Attribute::new("deposit_denom", msg.deposit_denom.clone()),
        Attribute::new("maxbtc_denom", msg.maxbtc_denom.clone()),
        Attribute::new("deposit_flush_period", msg.deposit_flush_period.to_string()),
        Attribute::new("deposit_cost", msg.deposit_cost.to_string()),
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
    assert!(!cfg.paused);
    // etc. check more fields
    assert_eq!(cfg.deposit_decimals, 6u32);
    assert_eq!(cfg.deposit_denom, "wBTC");

    // Finally, check time-based items
    let last_deposit_flush_time = LAST_DEPOSIT_FLUSH_TIME.load(&deps.storage).unwrap();
    assert_eq!(last_deposit_flush_time, env.block.time.seconds());
}

#[test]
fn test_first_deposit_success() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    deps.querier
        .set_token_supply(&cfg.maxbtc_denom, Uint128::zero());

    // deposit_amount = 1 wBTC => deposit_coin.amount = 1 * 10^6 = 1_000_000
    // deposit_cost = 1% => user effectively deposits 0.99 wBTC
    // exchange_rate = 1 => minted = 0.99 => minted.atomics() = 990_000
    let deposit_amount: Uint128 = Uint128::from(1_000_000u128);
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(deposit_amount.u128(), "wBTC")],
    );

    // Set the balance that the contract will see AFTER receiving the deposit.
    // This is crucial to avoid underflow when the contract subtracts the incoming deposit.
    deps.querier.set_balance(
        env.contract.address.as_ref(),
        &cfg.deposit_denom,
        deposit_amount,
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
    TOTAL_DEPOSITED
        .save(&mut deps.storage, &Uint128::from(110_000_000u128))
        .unwrap();
    deps.querier
        .set_token_supply(&cfg.maxbtc_denom, Uint128::zero());

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
    deps.querier.set_allowed_recipient(false);
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
        ContractError::AddressNotAllowed {} => (),
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
fn test_flush_guard_not_enough_time_elapsed() {
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Deposit buffer: 0.5 wBTC.
    let buffer = Uint128::new(500_000);
    deps.querier
        .set_balance(env.contract.address.as_ref(), "wBTC", buffer);

    // Set LAST_DEPOSIT_FLUSH_TIME so that *less* than `deposit_flush_period`
    // seconds have elapsed.
    let now = env.block.time.seconds();
    LAST_DEPOSIT_FLUSH_TIME
        .save(&mut deps.storage, &(now - (cfg.deposit_flush_period / 2)))
        .unwrap();

    let info = message_info(&deps.api.addr_make("flusher"), &[]);

    let resp = execute_flush_deposits(deps.as_mut(), env.clone(), info).unwrap();

    // Status attribute.
    let status_attr = resp
        .attributes
        .iter()
        .find(|a| a.key == "status")
        .expect("status attribute present");
    assert_eq!(status_attr.value, "not_enough_time_elapsed");

    // LAST_DEPOSIT_FLUSH_TIME unchanged.
    let stored = LAST_DEPOSIT_FLUSH_TIME.load(&deps.storage).unwrap();
    assert_eq!(stored, now - (cfg.deposit_flush_period / 2));
    // No transfer messages emitted.
    assert!(resp.messages.is_empty());
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

    (deps, env, info)
}

fn default_instantiate_msg(
    deps: &OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> InstantiateMsg {
    InstantiateMsg {
        owner: deps.api.addr_make("owner_addr").to_string(),
        aum_contract: deps.api.addr_make("aum_addr").to_string(),
        deposit_pump_contract: deps.api.addr_make("pump_addr").to_string(),
        treasury_address: deps.api.addr_make("treasury_addr").to_string(),
        exchange_rate_provider_contract: deps
            .api
            .addr_make("exchange_rate_provider_addr")
            .to_string(),
        deposit_denom: "wBTC".to_string(),
        deposit_decimals: 6u32,
        maxbtc_denom: "maxbtc".to_string(),
        deposit_flush_period: 3600,
        deposit_cost: Decimal::percent(1),
        deposits_cap: None,
        allowlist_contract: deps.api.addr_make("allow_list_addr").to_string(),
        fee_collector_params: FeeMinterParams {
            code_id: 0,                                       // Test
            salt: Binary::from(vec![1, 2, 3, 4]),             // Test
            fee_apy_reduction_percentage: Default::default(), // Test
            collection_period_seconds: 0,                     // Test
        },
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
