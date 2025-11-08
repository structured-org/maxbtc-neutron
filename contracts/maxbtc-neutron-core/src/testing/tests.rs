use std::str::FromStr;

use crate::contract::{execute, execute_tick, instantiate};
use crate::error::ContractError;
use crate::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockStorage};
use cosmwasm_std::{
    coin, to_json_binary, Attribute, BankMsg, Coin, CosmosMsg, Decimal, DepsMut, Env, Int256,
    MessageInfo, OwnedDeps, Response, SignedDecimal256, SubMsg, Uint128, Uint64, WasmMsg,
};
use cw_utils::PaymentError;
use maxbtc_base::msg::core::{ExecuteMsg, InstantiateMsg, WaitosaurObserverExecuteMsg};
use maxbtc_base::msg::{
    token::ExecuteMsg as TokenExecuteMsg, waitosaur_holder::ExecuteMsg as WaitosaurHolderExecuteMsg,
};
use maxbtc_base::state::{
    core::{
        Batch, ContractState, WaitosaurObserverState, ACTIVE_BATCH, CONFIG, FINALIZED_BATCHES, FSM,
        WITHDRAWING_BATCH,
    },
    waitosaur_holder::State as WaitsaurHolderState,
};
use neutron_std::types::osmosis::tokenfactory::v1beta1::{
    DenomAuthorityMetadata, QueryDenomAuthorityMetadataResponse,
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
}

#[test]
fn test_withdraw_success() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    // withdraw_amount = 1 maxBTC => withdraw_coin.amount = 1 * 10^6 = 1_000_000
    let withdraw_amount: Uint128 = Uint128::from(1_000_000u128);
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(
            withdraw_amount.u128(),
            "factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/maxbtc",
        )],
    );

    // Act
    let res = do_withdraw(deps.as_mut(), env.clone(), info.clone()).unwrap();

    // Assert
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.token_contract.to_string(),
            msg: to_json_binary(&TokenExecuteMsg::Burn {}).unwrap(),
            funds: vec![Coin {
                denom: "factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/maxbtc".to_string(),
                amount: withdraw_amount,
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.token_contract.to_string(),
                msg: to_json_binary(&TokenExecuteMsg::CreateRedemptionToken {
                    redemption_subdenom: "redemption/batch/1".to_string(),
                }).unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.token_contract.to_string(),
                msg: to_json_binary(&TokenExecuteMsg::Mint {
                    amount: Coin {
                        amount: withdraw_amount,
                        denom: "factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/redemption/batch/1".to_string().clone(),
                    },
                    recipient: info.sender.to_string(),
                }).unwrap(),
                funds: vec![],
            }))
        ]
    );

    assert_eq!(
        res.attributes,
        vec![
            Attribute::new("action".to_string(), "withdraw".to_string()),
            Attribute::new(
                "sender".to_string(),
                "cosmwasm1nf5ew8caktsasamnda7nd95s5wl40nezqajmchkwtswe6rx94wwsyqtmxc".to_string()
            ),
            Attribute::new("batch_id".to_string(), "1".to_string()),
            Attribute::new("withdraw_amount".to_string(), "1000000".to_string()),
        ]
    );

    let active_batch = ACTIVE_BATCH.load(&deps.storage).unwrap();
    assert_eq!(
        active_batch,
        Batch {
            batch_id: 1u64,
            btc_requested: Uint128::zero(),
            maxbtc_burned: withdraw_amount,
            collected_amount: Uint128::zero(),
            deposit_decimals: 6u32,
            collector_historical_balance: Uint128::zero(),
        }
    );
}

#[test]
fn test_withdraw_no_denom_creation() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    // withdraw_amount = 1 maxBTC => withdraw_coin.amount = 1 * 10^6 = 1_000_000
    let withdraw_amount: Uint128 = Uint128::from(1_000_000u128);
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(
            withdraw_amount.u128(),
            "factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/maxbtc",
        )],
    );

    deps.querier
        .set_denom_metadata(QueryDenomAuthorityMetadataResponse {
            authority_metadata: Some(DenomAuthorityMetadata {
                admin: "fee_collector_addr".to_string(),
            }),
        });

    // Set the total supply that the contract will check to create tokenfactory redemption denom.
    deps.querier.set_supply(
        "factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/redemption/batch/1",
        withdraw_amount,
    );

    // Act
    let res = do_withdraw(deps.as_mut(), env.clone(), info.clone()).unwrap();

    // Assert
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.token_contract.to_string(),
            msg: to_json_binary(&TokenExecuteMsg::Burn {}).unwrap(),
            funds: vec![Coin {
                denom: "factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/maxbtc".to_string(),
                amount: withdraw_amount,
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.token_contract.to_string(),
                msg: to_json_binary(&TokenExecuteMsg::Mint {
                    amount: Coin {
                        amount: withdraw_amount,
                        denom: "factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/redemption/batch/1".to_string().clone(),
                    },
                    recipient: info.sender.to_string(),
                }).unwrap(),
                funds: vec![],
            }))
        ]
    );

    assert_eq!(
        res.attributes,
        vec![
            Attribute::new("action".to_string(), "withdraw".to_string()),
            Attribute::new(
                "sender".to_string(),
                "cosmwasm1nf5ew8caktsasamnda7nd95s5wl40nezqajmchkwtswe6rx94wwsyqtmxc".to_string()
            ),
            Attribute::new("batch_id".to_string(), "1".to_string()),
            Attribute::new("withdraw_amount".to_string(), "1000000".to_string()),
        ]
    );

    let active_batch = ACTIVE_BATCH.load(&deps.storage).unwrap();
    assert_eq!(
        active_batch,
        Batch {
            batch_id: 1u64,
            btc_requested: Uint128::zero(),
            maxbtc_burned: withdraw_amount,
            collected_amount: Uint128::zero(),
            deposit_decimals: 6u32,
            collector_historical_balance: Uint128::zero(),
        }
    );
}

#[test]
fn test_withdraw_paused() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let mut cfg = CONFIG.load(&deps.storage).unwrap();
    cfg.paused = true;
    CONFIG.save(&mut deps.storage, &cfg).unwrap();

    // withdraw_amount = 1 maxBTC => withdraw_coin.amount = 1 * 10^6 = 1_000_000
    let withdraw_amount: Uint128 = Uint128::from(1_000_000u128);
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(
            withdraw_amount.u128(),
            "factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/maxbtc",
        )],
    );

    // Act
    let error = do_withdraw(deps.as_mut(), env.clone(), info.clone()).unwrap_err();

    // Assert
    match error {
        ContractError::ContractPaused {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_withdraw_not_allowlisted() {
    let (mut deps, env, _) = setup_contract();
    deps.querier.set_allowed_recipient(false);
    let withdraw_amount: Uint128 = Uint128::from(1_000_000u128);
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(
            withdraw_amount.u128(),
            "factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/maxbtc",
        )],
    );

    // Act
    let err = do_withdraw(deps.as_mut(), env.clone(), info.clone()).unwrap_err();

    // Assert
    match err {
        ContractError::AddressNotAllowed {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_withdraw_wrong_denom() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    // withdraw_amount = 1 maxBTC => withdraw_coin.amount = 1 * 10^6 = 1_000_000
    let withdraw_amount: Uint128 = Uint128::from(1_000_000u128);
    let info = message_info(
        &deps.api.addr_make("depositor"),
        &[coin(withdraw_amount.u128(), "wBTC")],
    );

    // Act
    let error = do_withdraw(deps.as_mut(), env.clone(), info.clone()).unwrap_err();

    // Assert
    match error {
        ContractError::PaymentError(PaymentError::MissingDenom(denom)) => {
            assert_eq!(denom, "factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/maxbtc");
        }
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_first_deposit_success() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    deps.querier
        .set_exchange_rate((Decimal::one(), env.block.time.seconds()));

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

    deps.querier
        .set_exchange_rate((Decimal::one(), env.block.time.seconds()));

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
fn test_deposit_exchange_rate_stale() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    deps.querier.set_exchange_rate((Decimal::one(), 0));

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
        None,
    );
    assert!(res.is_err());
    let err = res.err().unwrap();
    match err {
        ContractError::ERDataStale {} => {}
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

    deps.querier.set_aum_in_wbtc(Int256::from(110_000_000u128));

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
        ContractError::PaymentError(PaymentError::NoFunds {}) => (),
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
        ContractError::PaymentError(PaymentError::MultipleDenoms {}) => (),
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
        ContractError::PaymentError(PaymentError::NoFunds {}) => (),
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
        ContractError::PaymentError(PaymentError::MissingDenom(denom)) => {
            assert_eq!(denom, "wBTC");
        }
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_withdraw_pending_tick_collect_ceffu_amount() {
    let (mut deps, env, _) = setup_contract();

    FSM.set_initial_state(&mut deps.storage, ContractState::WithdrawPending)
        .unwrap();

    WITHDRAWING_BATCH
        .save(
            &mut deps.storage,
            &Some(Batch {
                batch_id: 1u64,
                btc_requested: Uint128::new(200_000u128),
                maxbtc_burned: Uint128::new(200_000u128),
                collected_amount: Uint128::new(150_000u128),
                deposit_decimals: 6u32,
                collector_historical_balance: Uint128::zero(),
            }),
        )
        .unwrap();

    let operator = deps.api.addr_make("operator_addr");

    let info = message_info(&operator, &[]);

    deps.querier
        .set_waitsaur_holder_state(WaitsaurHolderState::Locked {
            amount: Uint128::new(50_000u128),
            at_timestamp: 0,
        });

    let resp = execute_tick(deps.as_mut(), env.clone(), info).unwrap();

    assert_eq!(
        resp,
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr:
                    "cosmwasm1nylrq8x440yzqme262zy5875tt7vyn5yghjg5u807gms0359zl9svnrlrp"
                        .to_string(),
                msg: to_json_binary(&WaitosaurHolderExecuteMsg::Unlock {}).unwrap(),
                funds: vec![],
            }))
            .add_attribute("action", "tick")
            .add_attribute("stage", "withdraw_pending")
            .add_attribute("received_from_binance", "50000")
    );

    let withdrawing_batch = WITHDRAWING_BATCH.load(&deps.storage).unwrap();
    assert_eq!(
        withdrawing_batch,
        Some(Batch {
            batch_id: 1u64,
            btc_requested: Uint128::new(200_000u128),
            maxbtc_burned: Uint128::new(200_000u128),
            collected_amount: Uint128::new(200_000u128),
            deposit_decimals: 6u32,
            collector_historical_balance: Uint128::zero(),
        })
    );

    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::WithdrawNeutron);
}

#[test]
fn test_idle_tick_wrong_operator() {
    let (mut deps, env, _) = setup_contract();

    FSM.set_initial_state(&mut deps.storage, ContractState::Idle)
        .unwrap();

    let random_user = deps.api.addr_make("random_user");

    let info = message_info(&random_user, &[]);

    let resp_err = execute_tick(deps.as_mut(), env.clone(), info).unwrap_err();

    assert_eq!(resp_err, ContractError::Unauthorized {});
}

#[test]
fn test_idle_tick_withdraw_and_stay_idle() {
    let (mut deps, env, _) = setup_contract();

    deps.querier
        .set_exchange_rate((Decimal::from_str("0.95").unwrap(), env.block.time.seconds()));

    FSM.set_initial_state(&mut deps.storage, ContractState::Idle)
        .unwrap();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    deps.querier.set_balance(
        env.contract.address.as_ref(),
        &cfg.deposit_denom,
        Uint128::new(200_000u128),
    );

    let batch_id = 1u64;
    let burned_amount = Uint128::new(100_000u128);
    ACTIVE_BATCH
        .save(
            &mut deps.storage,
            &Batch {
                batch_id,
                btc_requested: Uint128::zero(),
                maxbtc_burned: burned_amount,
                collected_amount: Uint128::zero(),
                deposit_decimals: 6u32,
                collector_historical_balance: Uint128::zero(),
            },
        )
        .unwrap();

    let operator = deps.api.addr_make("operator_addr");

    let info = message_info(&operator, &[]);

    let resp = execute_tick(deps.as_mut(), env.clone(), info).unwrap();

    assert_eq!(
        resp,
        Response::new()
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: cfg.withdrawal_manager_contract.to_string(),
                amount: vec![Coin {
                    denom: cfg.deposit_denom,
                    amount: Uint128::new(95_000u128),
                }],
            }))
            .add_attribute("action", "tick")
            .add_attribute("stage", "idle")
            .add_attribute("amount", "95000")
            .add_attribute("covered_from_deposit", "95000")
            .add_attribute("new_batch_id", "2")
    );

    let active_batch = ACTIVE_BATCH.load(&deps.storage).unwrap();
    assert_eq!(
        active_batch,
        Batch {
            batch_id: 2u64,
            btc_requested: Uint128::zero(),
            maxbtc_burned: Uint128::zero(),
            collected_amount: Uint128::zero(),
            collector_historical_balance: Uint128::zero(),
            deposit_decimals: 6u32,
        }
    );

    let finalized_batch = FINALIZED_BATCHES.load(&deps.storage, 1u64).unwrap();
    assert_eq!(
        finalized_batch,
        Batch {
            batch_id,
            btc_requested: Uint128::new(95_000u128),
            maxbtc_burned: Uint128::new(100_000u128),
            collected_amount: Uint128::new(95_000u128),
            collector_historical_balance: Uint128::zero(),
            deposit_decimals: 6u32,
        }
    );

    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::Idle);
}

#[test]
fn test_deposit_neutron_tick_locked() {
    let (mut deps, env, _) = setup_contract();

    deps.querier
        .set_waitosaur_observer_state(WaitosaurObserverState::Locked {
            amount: SignedDecimal256::from(Decimal::from_atomics(500_000u64, 0).unwrap()),
            at_timestamp: env.block.time.seconds(),
        });

    FSM.set_initial_state(&mut deps.storage, ContractState::DepositNeutron)
        .unwrap();

    let operator = deps.api.addr_make("operator_addr");

    let info = message_info(&operator, &[]);

    let resp_err = execute_tick(deps.as_mut(), env.clone(), info).unwrap_err();

    assert_eq!(resp_err, ContractError::WaitosaurLocked {});
}

#[test]
fn test_ticks_cycle() {
    let (mut deps, env, _) = setup_contract();

    deps.querier
        .set_exchange_rate((Decimal::from_str("0.95").unwrap(), env.block.time.seconds()));

    FSM.set_initial_state(&mut deps.storage, ContractState::Idle)
        .unwrap();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    deps.querier.set_balance(
        env.contract.address.as_ref(),
        &cfg.deposit_denom,
        Uint128::new(180_000u128),
    );

    let batch_id = 1u64;
    let burned_amount = Uint128::new(200_000u128);
    ACTIVE_BATCH
        .save(
            &mut deps.storage,
            &Batch {
                batch_id,
                btc_requested: Uint128::zero(),
                maxbtc_burned: burned_amount,
                collected_amount: Uint128::zero(),
                deposit_decimals: 6u32,
                collector_historical_balance: Uint128::zero(),
            },
        )
        .unwrap();

    let operator = deps.api.addr_make("operator_addr");

    let info = message_info(&operator, &[]);

    let resp = execute_tick(deps.as_mut(), env.clone(), info).unwrap();

    assert_eq!(
        resp,
        Response::new()
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: cfg.withdrawal_manager_contract.to_string(),
                amount: vec![Coin {
                    denom: cfg.deposit_denom,
                    amount: Uint128::new(180_000u128),
                }],
            }))
            .add_attribute("action", "tick")
            .add_attribute("stage", "idle")
            .add_attribute("amount", "190000")
            .add_attribute("covered_from_deposit", "180000",)
            .add_attribute("new_batch_id", "2")
    );

    let active_batch = ACTIVE_BATCH.load(&deps.storage).unwrap();
    assert_eq!(
        active_batch,
        Batch {
            batch_id: 2u64,
            btc_requested: Uint128::zero(),
            maxbtc_burned: Uint128::zero(),
            collected_amount: Uint128::zero(),
            deposit_decimals: 6u32,
            collector_historical_balance: Uint128::zero(),
        }
    );

    let withdrawing_batch = WITHDRAWING_BATCH.load(&deps.storage).unwrap();
    assert_eq!(
        withdrawing_batch,
        Some(Batch {
            batch_id: 1u64,
            btc_requested: Uint128::new(190_000u128),
            maxbtc_burned: Uint128::new(200_000u128),
            collected_amount: Uint128::new(180_000u128),
            deposit_decimals: 6u32,
            collector_historical_balance: Uint128::zero(),
        })
    );

    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::WithdrawJLP);
}

#[test]
fn test_withdraw_ticks_cycle() {
    let (mut deps, env, _) = setup_contract();

    deps.querier
        .set_exchange_rate((Decimal::one(), env.block.time.seconds()));

    FSM.set_initial_state(&mut deps.storage, ContractState::Idle)
        .unwrap();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    deps.querier.set_balance(
        env.contract.address.as_ref(),
        &cfg.deposit_denom,
        Uint128::new(150_000u128),
    );

    let batch_id = 1u64;
    let burned_amount = Uint128::new(200_000u128);
    ACTIVE_BATCH
        .save(
            &mut deps.storage,
            &Batch {
                batch_id,
                btc_requested: Uint128::zero(),
                maxbtc_burned: burned_amount,
                collected_amount: Uint128::zero(),
                deposit_decimals: 6u32,
                collector_historical_balance: Uint128::zero(),
            },
        )
        .unwrap();

    let operator = deps.api.addr_make("operator_addr");

    let info = message_info(&operator, &[]);

    let resp = execute_tick(deps.as_mut(), env.clone(), info.clone()).unwrap();

    assert_eq!(
        resp,
        Response::new()
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: cfg.withdrawal_manager_contract.to_string(),
                amount: vec![Coin {
                    denom: cfg.deposit_denom,
                    amount: Uint128::new(150_000u128),
                }],
            }))
            .add_attribute("action", "tick")
            .add_attribute("stage", "idle")
            .add_attribute("amount", "200000")
            .add_attribute("covered_from_deposit", "150000")
            .add_attribute("new_batch_id", "2")
    );

    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::WithdrawJLP);

    execute_tick(deps.as_mut(), env.clone(), info.clone()).unwrap();
    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::WithdrawPending);

    deps.querier
        .set_waitsaur_holder_state(WaitsaurHolderState::Locked {
            amount: Uint128::new(50_000u128),
            at_timestamp: 0,
        });
    execute_tick(deps.as_mut(), env.clone(), info.clone()).unwrap();
    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::WithdrawNeutron);

    execute_tick(deps.as_mut(), env.clone(), info).unwrap();
    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::Idle);

    let withdrawing_batch = WITHDRAWING_BATCH.load(&deps.storage).unwrap();
    assert_eq!(withdrawing_batch, None);

    let finalized_batch = FINALIZED_BATCHES.load(&deps.storage, 1u64).unwrap();
    assert_eq!(
        finalized_batch,
        Batch {
            batch_id,
            btc_requested: Uint128::new(200_000u128),
            maxbtc_burned: Uint128::new(200_000u128),
            collected_amount: Uint128::new(200_000u128),
            deposit_decimals: 6u32,
            collector_historical_balance: Uint128::zero(),
        }
    );
}

#[test]
fn test_idle_tick_goes_to_deposit_neutron() {
    let (mut deps, env, _) = setup_contract();

    FSM.set_initial_state(&mut deps.storage, ContractState::Idle)
        .unwrap();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    deps.querier.set_balance(
        env.contract.address.as_ref(),
        &cfg.deposit_denom,
        Uint128::new(200_000u128),
    );

    let operator = deps.api.addr_make("operator_addr");

    let info = message_info(&operator, &[]);

    let resp = execute_tick(deps.as_mut(), env.clone(), info).unwrap();

    assert_eq!(
        resp.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: cfg.deposit_forwarder_contract.to_string(),
                amount: vec![Coin {
                    denom: cfg.deposit_denom,
                    amount: Uint128::new(200_000),
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.waitosaur_observer_contract.to_string(),
                msg: to_json_binary(&WaitosaurObserverExecuteMsg::Lock {
                    amount: SignedDecimal256::from(Decimal::from_atomics(200_000u64, 0).unwrap()),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    assert_eq!(
        resp.attributes,
        vec![
            Attribute::new("action".to_string(), "flush_deposits".to_string()),
            Attribute::new("sender".to_string(), operator.to_string()),
            Attribute::new("flushed".to_string(), "200000wBTC".to_string()),
        ]
    );

    let current_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(current_state, ContractState::DepositNeutron);
}

#[test]
fn test_ticks_deposit_cycle() {
    let (mut deps, env, _) = setup_contract();

    FSM.set_initial_state(&mut deps.storage, ContractState::Idle)
        .unwrap();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    deps.querier.set_balance(
        env.contract.address.as_ref(),
        &cfg.deposit_denom,
        Uint128::new(200_000u128),
    );

    let operator = deps.api.addr_make("operator_addr");

    let info = message_info(&operator, &[]);

    let resp = execute_tick(deps.as_mut(), env.clone(), info.clone()).unwrap();

    assert_eq!(
        resp.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: cfg.deposit_forwarder_contract.to_string(),
                amount: vec![Coin {
                    denom: cfg.deposit_denom,
                    amount: Uint128::new(200_000),
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.waitosaur_observer_contract.to_string(),
                msg: to_json_binary(&WaitosaurObserverExecuteMsg::Lock {
                    amount: SignedDecimal256::from(
                        Decimal::from_atomics(Uint128::new(200_000u128), 0).unwrap()
                    ),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    assert_eq!(
        resp.attributes,
        vec![
            Attribute::new("action".to_string(), "flush_deposits".to_string()),
            Attribute::new("sender".to_string(), operator.to_string()),
            Attribute::new("flushed".to_string(), "200000wBTC".to_string()),
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
        operator: deps.api.addr_make("operator_addr").to_string(),
        token_contract: deps.api.addr_make("token_contract_addr").to_string(),
        factory_contract: deps.api.addr_make("factory_contract_addr").to_string(),
        deposit_forwarder_contract: deps.api.addr_make("forwarder_addr").to_string(),
        exchange_rate_provider_contract: deps
            .api
            .addr_make("exchange_rate_provider_addr")
            .to_string(),
        exchange_rate_stale_period: Uint64::new(60),
        deposit_denom: "wBTC".to_string(),
        deposit_decimals: 6u32,
        deposit_cost: Decimal::percent(1),
        deposits_cap: None,
        allowlist_contract: deps.api.addr_make("allow_list_addr").to_string(),
        fee_collector_contract: deps.api.addr_make("fee_collector_addr").to_string(),
        waitosaur_observer_contract: deps.api.addr_make("waitosaur_addr").to_string(),
        waitosaur_holder_contract: deps.api.addr_make("waitosaur_holder_contract").to_string(),
        withdrawal_manager_contract: deps.api.addr_make("withdrawal_manager_addr").to_string(),
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

/// A convenience helper for calling the `execute_withdraw` entry point.
fn do_withdraw(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    execute(deps, env, info, ExecuteMsg::Withdraw {})
}
