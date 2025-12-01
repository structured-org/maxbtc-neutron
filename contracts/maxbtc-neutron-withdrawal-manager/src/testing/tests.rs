use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use crate::testing::mock_querier::mock_dependencies;

use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockStorage};
use cosmwasm_std::{
    coin, from_json, to_json_binary, Addr, Attribute, BankMsg, Coin, CosmosMsg, Env, OwnedDeps,
    StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw_ownable::{
    Action,
    OwnershipError::{NotOwner, NotPendingOwner},
};
use maxbtc_base::msg::{
    token::ExecuteMsg as TokenExecuteMsg,
    withdrawal_manager::{ExecuteMsg, InstantiateMsg, QueryMsg},
};
use maxbtc_base::state::withdrawal_manager::PAID_AMOUNT;
use maxbtc_base::state::{
    core::Batch,
    withdrawal_manager::{Config, Pause, CONFIG, PAUSE},
};

#[test]
fn proper_initialization() {
    let (deps, _env) = setup_contract();

    let config = CONFIG.load(&deps.storage).unwrap();
    let expected_config = Config {
        factory_contract: deps.api.addr_make("factory_contract"),
        core_contract: deps.api.addr_make("core_contract"),
        token_contract: deps.api.addr_make("token_contract"),
        deposit_denom: "deposit_denom".to_string(),
    };
    let owner = deps.api.addr_make("owner");
    cw_ownable::assert_owner(&deps.storage, &owner).unwrap();
    assert_config_equals(&config, &expected_config);
}

#[test]
fn test_instantiate_with_invalid_owner() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");

    let msg = InstantiateMsg {
        owner: "invalid...address...".to_string(),
        factory_contract: deps.api.addr_make("factory_contract").into_string(),
        core_contract: deps.api.addr_make("core_contract").into_string(),
        token_contract: deps.api.addr_make("token_contract").into_string(),
        deposit_denom: "deposit_denom".to_string(),
    };
    let info = message_info(&owner, &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(matches!(
        err,
        ContractError::Std(StdError::GenericErr { .. })
    ));
}

#[test]
fn test_instantiate_with_invalid_factory_contract() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");

    let msg = InstantiateMsg {
        owner: owner.to_string(),
        factory_contract: "invalid...address...".to_string(),
        core_contract: deps.api.addr_make("core_contract").into_string(),
        token_contract: deps.api.addr_make("token_contract").into_string(),
        deposit_denom: "deposit_denom".to_string(),
    };
    let info = message_info(&owner, &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(matches!(
        err,
        ContractError::Std(StdError::GenericErr { .. })
    ));
}

#[test]
fn test_instantiate_with_invalid_core_contract() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");

    let msg = InstantiateMsg {
        owner: owner.to_string(),
        factory_contract: deps.api.addr_make("factory_contract").into_string(),
        core_contract: "invalid...address...".to_string(),
        token_contract: deps.api.addr_make("token_contract").into_string(),
        deposit_denom: "deposit_denom".to_string(),
    };
    let info = message_info(&owner, &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(matches!(
        err,
        ContractError::Std(StdError::GenericErr { .. })
    ));
}

#[test]
fn test_instantiate_with_invalid_token_contract() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");

    let msg = InstantiateMsg {
        owner: owner.to_string(),
        factory_contract: deps.api.addr_make("factory_contract").into_string(),
        core_contract: deps.api.addr_make("core_contract").into_string(),
        token_contract: "invalid...address...".to_string(),
        deposit_denom: "deposit_denom".to_string(),
    };
    let info = message_info(&owner, &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(matches!(
        err,
        ContractError::Std(StdError::GenericErr { .. })
    ));
}

#[test]
fn test_ownership() {
    let mut deps = cosmwasm_std::testing::mock_dependencies();
    let env = mock_env();
    let first_owner = deps.api.addr_make("first_owner");
    let second_owner = deps.api.addr_make("second_owner");
    let other = deps.api.addr_make("other");

    let owner_info = message_info(&first_owner, &[]);
    let second_owner_info = message_info(&second_owner, &[]);
    let other_info = message_info(&other, &[]);

    // Test empty vector for messengers
    let msg = InstantiateMsg {
        owner: first_owner.to_string(),
        factory_contract: deps.api.addr_make("factory_contract").into_string(),
        core_contract: deps.api.addr_make("core_contract").into_string(),
        token_contract: deps.api.addr_make("token_contract").into_string(),
        deposit_denom: "deposit_denom".to_string(),
    };
    let result = instantiate(deps.as_mut(), env.clone(), owner_info.clone(), msg);
    assert!(result.is_ok());

    // owner is written in instantiate
    let ownership = cw_ownable::get_ownership(&deps.storage).unwrap();
    assert_eq!(ownership.owner.unwrap(), first_owner);

    // non-owner cannot transfer ownership
    let update_msg_1 = ExecuteMsg::UpdateOwnership(Action::TransferOwnership {
        new_owner: other.to_string(),
        expiry: None,
    });
    let result = execute(deps.as_mut(), env.clone(), other_info.clone(), update_msg_1).unwrap_err();
    assert_eq!(result, ContractError::OwnershipError(NotOwner));

    // transfer ownership writes pending owner
    let update_msg_2 = ExecuteMsg::UpdateOwnership(Action::TransferOwnership {
        new_owner: second_owner.to_string(),
        expiry: None,
    });
    let result = execute(deps.as_mut(), env.clone(), owner_info, update_msg_2);
    assert!(result.is_ok());
    let ownership = cw_ownable::get_ownership(&deps.storage).unwrap();
    assert_eq!(ownership.owner.unwrap(), first_owner);
    assert_eq!(ownership.pending_owner.unwrap(), second_owner);

    // other user cannot accept ownership
    let update_msg_3 = ExecuteMsg::UpdateOwnership(Action::AcceptOwnership {});
    let result = execute(deps.as_mut(), env.clone(), other_info, update_msg_3).unwrap_err();
    assert_eq!(result, ContractError::OwnershipError(NotPendingOwner));

    // pending owner can accept ownership
    let update_msg_4 = ExecuteMsg::UpdateOwnership(Action::AcceptOwnership {});
    let result = execute(deps.as_mut(), env.clone(), second_owner_info, update_msg_4);
    assert!(result.is_ok());
    let ownership = cw_ownable::get_ownership(&deps.storage).unwrap();
    assert_eq!(ownership.owner.unwrap(), second_owner);
}

#[test]
fn update_config_by_owner() {
    let (mut deps, _env) = setup_contract();
    let owner = deps.api.addr_make("owner");
    let msg = ExecuteMsg::UpdateConfig {
        factory_contract: Some(deps.api.addr_make("new_factory_contract").into_string()),
        core_contract: Some(deps.api.addr_make("new_core_contract").into_string()),
        token_contract: Some(deps.api.addr_make("new_token_contract").into_string()),
        deposit_denom: Some("new_deposit_denom".to_string()),
    };

    let res = execute_msg(&mut deps, mock_env(), &owner, msg, vec![]).unwrap();
    assert_eq!(0, res.messages.len());

    let config = CONFIG.load(&deps.storage).unwrap();
    let expected_config = Config {
        factory_contract: deps.api.addr_make("new_factory_contract"),
        core_contract: deps.api.addr_make("new_core_contract"),
        token_contract: deps.api.addr_make("new_token_contract"),
        deposit_denom: "new_deposit_denom".to_string(),
    };
    assert_config_equals(&config, &expected_config);
}

#[test]
fn update_config_by_unauthorized() {
    let (mut deps, _env) = setup_contract();
    let oracle1 = deps.api.addr_make("oracle1");

    let msg = ExecuteMsg::UpdateConfig {
        factory_contract: Some(deps.api.addr_make("new_factory_contract").into_string()),
        core_contract: Some(deps.api.addr_make("new_core_contract").into_string()),
        token_contract: Some(deps.api.addr_make("new_token_contract").into_string()),
        deposit_denom: Some("new_deposit_denom".to_string()),
    };

    let err = execute_msg(&mut deps, mock_env(), &oracle1, msg, vec![]).unwrap_err();
    assert_eq!(err, ContractError::OwnershipError(NotOwner));

    let config = CONFIG.load(&deps.storage).unwrap();
    let expected_config = Config {
        factory_contract: deps.api.addr_make("factory_contract"),
        core_contract: deps.api.addr_make("core_contract"),
        token_contract: deps.api.addr_make("token_contract"),
        deposit_denom: "deposit_denom".to_string(),
    };
    assert_config_equals(&config, &expected_config);
}

#[test]
fn query_config() {
    let (deps, _env) = setup_contract();
    let bin = query_msg(&deps, mock_env(), QueryMsg::Config {}).unwrap();
    let config: Config = from_json(bin).unwrap();

    let expected_config = Config {
        factory_contract: deps.api.addr_make("factory_contract"),
        core_contract: deps.api.addr_make("core_contract"),
        token_contract: deps.api.addr_make("token_contract"),
        deposit_denom: "deposit_denom".to_string(),
    };
    assert_config_equals(&config, &expected_config);
}

#[test]
fn update_set_pause_by_owner() {
    let (mut deps, _env) = setup_contract();
    let owner = deps.api.addr_make("owner");
    let msg = ExecuteMsg::SetPause {
        pause: Pause {
            receive_nft_withdraw: true,
        },
    };

    let res = execute_msg(&mut deps, mock_env(), &owner, msg, vec![]).unwrap();
    assert_eq!(0, res.messages.len());

    let pause = PAUSE.load(&deps.storage).unwrap();

    assert_eq!(
        &pause,
        &Pause {
            receive_nft_withdraw: true
        }
    );
}

#[test]
fn update_set_pause_by_unauthorized() {
    let (mut deps, _env) = setup_contract();
    let oracle1 = deps.api.addr_make("oracle1");

    let msg = ExecuteMsg::SetPause {
        pause: Pause {
            receive_nft_withdraw: true,
        },
    };

    let err = execute_msg(&mut deps, mock_env(), &oracle1, msg, vec![]).unwrap_err();
    assert_eq!(err, ContractError::OwnershipError(NotOwner));

    let pause = PAUSE.load(&deps.storage).unwrap();

    assert_eq!(
        &pause,
        &Pause {
            receive_nft_withdraw: false
        }
    );
}

#[test]
fn query_pause() {
    let (deps, _env) = setup_contract();
    let bin = query_msg(&deps, mock_env(), QueryMsg::Pause {}).unwrap();
    let pause: Pause = from_json(bin).unwrap();

    assert_eq!(
        &pause,
        &Pause {
            receive_nft_withdraw: false
        }
    );
}

#[test]
fn test_claim_success() {
    // Arrange
    let (mut deps, env) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    let claim_amount: Uint128 = Uint128::from(100_000u128);

    let finalized_batch = Batch {
        batch_id: 1u64,
        btc_requested: Uint128::new(95_000u128),
        maxbtc_burned: Uint128::new(100_000u128),
        collected_amount: Uint128::new(95_000u128),
        deposit_decimals: 6u32,
        collector_historical_balance: Uint128::zero(),
    };

    deps.querier.update_wasm(
        deps.api.addr_make("core_contract").to_string(),
        to_json_binary(&finalized_batch).unwrap(),
    );

    // Set the total supply that the contract will check to create tokenfactory redemption denom.
    deps.querier.set_supply(
        "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/redemption/batch/1",
        claim_amount,
    );

    // Set the balance that the contract will see AFTER receiving the deposit.
    // This is crucial to avoid underflow when the contract subtracts the incoming deposit.
    deps.querier.set_balance(
        env.contract.address.as_ref(),
        &cfg.deposit_denom,
        claim_amount,
    );

    let sender = deps.api.addr_make("sender");

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let res = execute_msg(
        &mut deps,
        env.clone(),
        &sender,
        ExecuteMsg::Claim { recipient: recipient.clone() },
        vec![coin(
            claim_amount.u128(),
            "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/redemption/batch/1",
        )],
    )
    .unwrap();

    // Assert
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: recipient.to_string(),
                amount: vec![Coin {
                    denom: cfg.deposit_denom.clone(),
                    amount: Uint128::new(95_000u128),
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.token_contract.to_string(),
                msg: to_json_binary(&TokenExecuteMsg::Burn {}).unwrap(),
                funds: vec![coin(
                    claim_amount.u128(),
                    "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/redemption/batch/1",
                )],
            })),
        ]
    );

    assert_eq!(
        res.attributes,
        vec![
            Attribute::new("action".to_string(), "claim".to_string()),
            Attribute::new("batch_id".to_string(), "1".to_string()),
            Attribute::new("user_claim_btc".to_string(), "95000".to_string()),
        ]
    );

    let paid_amount = PAID_AMOUNT.load(&deps.storage, 1u64).unwrap();
    assert_eq!(paid_amount, Uint128::new(95_000u128));
}

#[test]
fn test_claim_part_of_the_batch_success() {
    // Arrange
    let (mut deps, env) = setup_contract();

    let cfg = CONFIG.load(&deps.storage).unwrap();

    let claim_amount: Uint128 = Uint128::from(50_000u128);

    let finalized_batch = Batch {
        batch_id: 1u64,
        btc_requested: Uint128::new(100_000u128),
        maxbtc_burned: Uint128::new(100_000u128),
        collected_amount: Uint128::new(100_000u128),
        deposit_decimals: 6u32,
        collector_historical_balance: Uint128::zero(),
    };

    deps.querier.update_wasm(
        deps.api.addr_make("core_contract").to_string(),
        to_json_binary(&finalized_batch).unwrap(),
    );

    // Set the total supply that the contract will check to create tokenfactory redemption denom.
    deps.querier.set_supply(
        "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/redemption/batch/1",
        claim_amount * Uint128::new(2u128),
    );

    // Set the balance that the contract will see AFTER receiving the deposit.
    // This is crucial to avoid underflow when the contract subtracts the incoming deposit.
    deps.querier.set_balance(
        env.contract.address.as_ref(),
        &cfg.deposit_denom,
        claim_amount,
    );

    let sender = deps.api.addr_make("sender");

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let res = execute_msg(
        &mut deps,
        env.clone(),
        &sender,
        ExecuteMsg::Claim { recipient: recipient.clone() },
        vec![coin(
            claim_amount.u128(),
            "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/redemption/batch/1",
        )],
    )
    .unwrap();

    // Assert
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: recipient.to_string(),
                amount: vec![Coin {
                    denom: cfg.deposit_denom.clone(),
                    amount: Uint128::new(50_000u128),
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.token_contract.to_string(),
                msg: to_json_binary(&TokenExecuteMsg::Burn {}).unwrap(),
                funds: vec![coin(
                    claim_amount.u128(),
                    "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/redemption/batch/1",
                )],
            })),
        ]
    );

    assert_eq!(
        res.attributes,
        vec![
            Attribute::new("action".to_string(), "claim".to_string()),
            Attribute::new("batch_id".to_string(), "1".to_string()),
            Attribute::new("user_claim_btc".to_string(), "50000".to_string()),
        ]
    );

    let paid_amount = PAID_AMOUNT.load(&deps.storage, 1u64).unwrap();
    assert_eq!(paid_amount, Uint128::new(50_000u128));
}

#[test]
fn test_claim_paused() {
    // Arrange
    let (mut deps, env) = setup_contract();

    let mut pause = PAUSE.load(&deps.storage).unwrap();
    pause.receive_nft_withdraw = true;
    PAUSE.save(&mut deps.storage, &pause).unwrap();

    // Act
    let sender = deps.api.addr_make("sender");
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let error = execute_msg(
        &mut deps,
        env.clone(),
        &sender,
        ExecuteMsg::Claim { recipient: recipient.clone() },
        vec![coin(
            100000u128,
            "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/redemption/batch/1",
        )],
    )
    .unwrap_err();

    // Assert
    match error {
        ContractError::ContractPaused {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_claim_no_token_funds() {
    // Arrange
    let (mut deps, env) = setup_contract();

    // Act
    let sender = deps.api.addr_make("sender");
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let error = execute_msg(
        &mut deps,
        env.clone(),
        &sender,
        ExecuteMsg::Claim {
            recipient: recipient.clone(),
        },
        vec![],
    )
    .unwrap_err();

    // Assert
    match error {
        ContractError::WrongRedemptionTokenOrNoFunds {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_claim_wrong_redemption_token() {
    // Arrange
    let (mut deps, env) = setup_contract();

    let claim_amount: Uint128 = Uint128::from(100_000u128);

    // Act
    let sender = deps.api.addr_make("sender");
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let error = execute_msg(
        &mut deps,
        env.clone(),
        &sender,
        ExecuteMsg::Claim {
            recipient: recipient.clone(),
        },
        vec![coin(
            claim_amount.u128(),
            "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/maxbtc",
        )],
    )
    .unwrap_err();

    // Assert
    match error {
        ContractError::WrongRedemptionTokenOrNoFunds {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_claim_wrong_tokens_amount() {
    // Arrange
    let (mut deps, env) = setup_contract();

    let claim_amount: Uint128 = Uint128::from(100_000u128);

    // Act
    let sender = deps.api.addr_make("sender");
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let error = execute_msg(
        &mut deps,
        env.clone(),
        &sender,
        ExecuteMsg::Claim { recipient: recipient.clone() },
        vec![coin(
            claim_amount.u128(),
            "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/redemption/batch/1",
            ),
            coin(
            claim_amount.u128(),
            "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/maxbtc",
            )],
    )
    .unwrap_err();

    // Assert
    match error {
        ContractError::WrongRedemptionTokenOrNoFunds {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_claim_batch_not_found() {
    // Arrange
    let (mut deps, env) = setup_contract();

    deps.querier.update_wasm(
        deps.api.addr_make("core_contract").to_string(),
        to_json_binary::<Vec<Batch>>(&vec![]).unwrap(),
    );

    let claim_amount: Uint128 = Uint128::from(100_000u128);

    // Act
    let sender = deps.api.addr_make("sender");
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let error = execute_msg(
        &mut deps,
        env.clone(),
        &sender,
        ExecuteMsg::Claim { recipient: recipient.clone() },
        vec![coin(
            claim_amount.u128(),
            "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/redemption/batch/1",
        )],
    )
    .unwrap_err();

    // Assert
    match error {
        ContractError::BatchIsNotWithdrawn {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_claim_supply_mismatch() {
    // Arrange
    let (mut deps, env) = setup_contract();

    let claim_amount: Uint128 = Uint128::from(100_000u128);

    let finalized_batch = Batch {
        batch_id: 1u64,
        btc_requested: Uint128::new(95_000u128),
        maxbtc_burned: Uint128::new(100_000u128),
        collected_amount: Uint128::new(95_000u128),
        deposit_decimals: 6u32,
        collector_historical_balance: Uint128::zero(),
    };
    deps.querier.update_wasm(
        deps.api.addr_make("core_contract").to_string(),
        to_json_binary(&finalized_batch).unwrap(),
    );

    // Act
    let sender = deps.api.addr_make("sender");
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let error = execute_msg(
        &mut deps,
        env.clone(),
        &sender,
        ExecuteMsg::Claim { recipient: recipient.clone() },
        vec![coin(
            claim_amount.u128(),
            "factory/cosmwasm1k2mr5h0a6296pe7s7hwttxzvls049wml8zxnpul3apufzu4qwvwsu8c5mn/redemption/batch/1",
        )],
    )
    .unwrap_err();

    // Assert
    match error {
        ContractError::RedemptionSupplyMismatch {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

// ============================================================================
// Test Helper Functions
// ============================================================================

/// Creates a contract with custom mock querier
fn setup_contract() -> (
    OwnedDeps<MockStorage, MockApi, crate::testing::mock_querier::WasmMockQuerier>,
    Env,
) {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");

    let msg = InstantiateMsg {
        owner: deps.api.addr_make("owner").to_string(),
        factory_contract: deps.api.addr_make("factory_contract").into_string(),
        core_contract: deps.api.addr_make("core_contract").into_string(),
        token_contract: deps.api.addr_make("token_contract").into_string(),
        deposit_denom: "deposit_denom".to_string(),
    };
    let env = mock_env();
    let info = message_info(&owner, &[]);
    let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());
    (deps, env)
}

/// Execute message helper
fn execute_msg<T>(
    deps: &mut OwnedDeps<MockStorage, MockApi, T>,
    env: Env,
    sender: &Addr,
    msg: ExecuteMsg,
    funds: Vec<Coin>,
) -> Result<cosmwasm_std::Response, ContractError>
where
    T: cosmwasm_std::Querier,
{
    let info = message_info(sender, &funds);
    execute(deps.as_mut(), env, info, msg)
}

/// Query helper
fn query_msg<T>(
    deps: &OwnedDeps<MockStorage, MockApi, T>,
    env: Env,
    msg: QueryMsg,
) -> StdResult<cosmwasm_std::Binary>
where
    T: cosmwasm_std::Querier,
{
    query(deps.as_ref(), env, msg)
}

/// Config verification helper
fn assert_config_equals(config: &Config, expected_config: &Config) {
    assert_eq!(config.factory_contract, expected_config.factory_contract);
    assert_eq!(config.core_contract, expected_config.core_contract);
    assert_eq!(config.token_contract, expected_config.token_contract);
    assert_eq!(config.deposit_denom, expected_config.deposit_denom);
}
