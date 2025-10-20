use crate::contract::{execute, execute_tick, instantiate};
use crate::error::ContractError;
use crate::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockStorage};
use cosmwasm_std::{
    coin, Attribute, BankMsg, Coin, CosmosMsg, Decimal, DepsMut, Env, MessageInfo, OwnedDeps,
    Response, SubMsg, Uint128,
};
use maxbtc_base::msg::core::{ExecuteMsg, InstantiateMsg};
use maxbtc_base::state::core::{
    ContractState, CONFIG, FSM, LAST_DEPOSIT_FLUSH_TIME, TOTAL_DEPOSITED,
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

    // Assert: check attributes
    let expected_attributes = vec![
        Attribute::new("action", "instantiate"),
        Attribute::new("owner", msg.owner.clone()),
        Attribute::new("allowlist_contract", msg.allowlist_contract.clone()),
        Attribute::new("deposit_denom", msg.deposit_denom.clone()),
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
        None,
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
fn test_deposit_less_than_expected() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

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
        Some(Uint128::from(1_000_001u128)),
    );
    assert!(res.is_err());
    let err = res.err().unwrap();
    match err {
        ContractError::SlippageLimitExceeded { requested, actual } => {
            assert_eq!(requested, 1_000_001u128);
            assert_eq!(actual, 990_000u128);
        }
        e => panic!("Unexpected error: {e:?}"),
    }
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
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient, None)
        .expect_err("Should error if contract paused");

    // Assert
    match err {
        ContractError::ContractPaused {} => (),
        e => panic!("Unexpected error: {e:?}"),
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

    // Provide a deposit
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(1_000_000u128, "wBTC")],
    ); // 1 wBTC

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient, None)
        .expect_err("Should exceed deposit cap");

    // Assert
    match err {
        ContractError::DepositCapExceeded {} => (),
        e => panic!("Unexpected error: {e:?}"),
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
        None,
    )
    .expect_err("Should error if depositor not in allowlist");

    // Assert
    match err {
        ContractError::AddressNotAllowed {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_deposit_no_funds() {
    let (mut deps, env, _) = setup_contract();
    let info = message_info(&deps.api.addr_make("depositor"), &[]); // no funds

    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient, None)
        .expect_err("No funds means error");

    match err {
        ContractError::NoFundsSent {} => (),
        e => panic!("Unexpected error: {e:?}"),
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
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient, None)
        .expect_err("Must fail if multiple funds are attached");

    match err {
        ContractError::InvalidDepositAmount {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_deposit_zero_amount() {
    let (mut deps, env, _) = setup_contract();
    // deposit 0 wBTC
    let info = message_info(&deps.api.addr_make("depositor"), &[coin(0u128, "wBTC")]);

    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient, None)
        .expect_err("Zero deposit is invalid");

    match err {
        ContractError::InvalidDepositAmount {} => (),
        e => panic!("Unexpected error: {e:?}"),
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
    let err = do_deposit(deps.as_mut(), env.clone(), info.clone(), recipient, None)
        .expect_err("Wrong denom should fail");

    match err {
        ContractError::InvalidDepositDenom { expected, received } => {
            assert_eq!(expected, "wBTC");
            assert_eq!(received, "ETH");
        }
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_idle_tick() {
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();
    FSM.set_initial_state(&mut deps.storage, ContractState::Idle)
        .unwrap();

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

    let owner = deps.api.addr_make("owner");
    cw_ownable::initialize_owner(&mut deps.storage, &deps.api, Some(owner.as_str())).unwrap();

    let info = message_info(&owner, &[]);

    let resp_err = execute_tick(deps.as_mut(), env.clone(), info).unwrap_err();

    assert_eq!(resp_err, ContractError::NotEnoughTimeElapsed {});
}

#[test]
fn test_ticks_cycle() {
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();
    FSM.set_initial_state(&mut deps.storage, ContractState::Idle)
        .unwrap();

    // Deposit buffer: 0.5 wBTC.
    let buffer = Uint128::new(500_000);
    deps.querier
        .set_balance(env.contract.address.as_ref(), "wBTC", buffer);

    // Set LAST_DEPOSIT_FLUSH_TIME so that *less* than `deposit_flush_period`
    // seconds have elapsed.
    let now = env.block.time.seconds();
    LAST_DEPOSIT_FLUSH_TIME
        .save(&mut deps.storage, &(now - (cfg.deposit_flush_period + 10)))
        .unwrap();

    let owner = deps.api.addr_make("owner");
    cw_ownable::initialize_owner(&mut deps.storage, &deps.api, Some(owner.as_str())).unwrap();

    let info = message_info(&owner, &[]);

    let resp = execute_tick(deps.as_mut(), env.clone(), info.clone()).unwrap();

    println!("{:?}", resp.attributes);

    assert_eq!(
        resp.messages,
        vec![SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: cfg.deposit_forwarder_contract.to_string(),
            amount: vec![Coin {
                denom: cfg.deposit_denom,
                amount: Uint128::new(500_000),
            }],
        }))]
    );

    assert_eq!(
        resp.attributes,
        vec![
            Attribute::new("action".to_string(), "flush_deposits".to_string()),
            Attribute::new("sender".to_string(), owner.to_string()),
            Attribute::new("flushed".to_string(), "500000".to_string()),
        ]
    );

    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::DepositNeutron);

    execute_tick(deps.as_mut(), env.clone(), info.clone()).unwrap();
    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::DepositPending);

    execute_tick(deps.as_mut(), env.clone(), info.clone()).unwrap();
    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::DepositJLP);

    execute_tick(deps.as_mut(), env.clone(), info).unwrap();
    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::Idle);
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
        token_contract: deps.api.addr_make("token_contract_addr").to_string(),
        factory_contract: deps.api.addr_make("factory_contract_addr").to_string(),
        deposit_forwarder_contract: deps.api.addr_make("forwarder_addr").to_string(),
        exchange_rate_provider_contract: deps
            .api
            .addr_make("exchange_rate_provider_addr")
            .to_string(),
        deposit_denom: "wBTC".to_string(),
        deposit_decimals: 6u32,
        deposit_flush_period: 3600,
        deposit_cost: Decimal::percent(1),
        deposits_cap: None,
        allowlist_contract: deps.api.addr_make("allow_list_addr").to_string(),
        fee_collector_contract: deps.api.addr_make("fee_collector_addr").to_string(),
        last_deposit_flush_time: None,
        total_deposited: None,
    }
}

/// A convenience helper for calling the `execute_deposit` entry point.
fn do_deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    min_receive_amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    execute(
        deps,
        env,
        info,
        ExecuteMsg::Deposit {
            recipient: recipient.to_string(),
            min_receive_amount,
        },
    )
}
