use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use crate::testing::mock_querier::mock_dependencies;

use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockStorage};
use cosmwasm_std::{
    from_json, Addr, BankMsg, Coin, CosmosMsg, Env, OwnedDeps, StdError, SubMsg, Uint128,
};
use cw_ownable::Action;
use cw_ownable::OwnershipError::{NotOwner, NotPendingOwner};
use maxbtc_base::msg::waitosaur_holder::{ExecuteMsg, InstantiateMsg, QueryMsg, UpdateConfig};
use maxbtc_base::state::waitosaur_holder::{Config, State, CONFIG};

#[test]
fn proper_initialization() {
    let (deps, _env) = setup_contract();

    let config = CONFIG.load(&deps.storage).unwrap();
    let expected_config = Config {
        unlocker: deps.api.addr_make("unlocker"),
        locker: deps.api.addr_make("locker"),
        withdraw_manager_contract: deps.api.addr_make("withdraw_manager_contract"),
        asset: "asset".to_string(),
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
        config: Config {
            locker: deps.api.addr_make("locker"),
            unlocker: deps.api.addr_make("unlocker"),
            withdraw_manager_contract: deps.api.addr_make("withdraw_manager_contract"),
            asset: "asset".to_string(),
        },
    };
    let info = message_info(&owner, &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(matches!(
        err,
        ContractError::Std(StdError::GenericErr { .. })
    ));
}

#[test]
fn test_instantiate_with_invalid_locker() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");

    let msg = InstantiateMsg {
        owner: owner.to_string(),
        config: Config {
            locker: Addr::unchecked("invalid...address..."),
            unlocker: deps.api.addr_make("unlocker"),
            withdraw_manager_contract: deps.api.addr_make("withdraw_manager_contract"),
            asset: "asset".to_string(),
        },
    };
    let info = message_info(&owner, &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(matches!(
        err,
        ContractError::Std(StdError::GenericErr { .. })
    ));
}

#[test]
fn test_instantiate_with_invalid_unlocker() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");

    let msg = InstantiateMsg {
        owner: owner.to_string(),
        config: Config {
            locker: deps.api.addr_make("locker"),
            unlocker: Addr::unchecked("invalid...address..."),
            withdraw_manager_contract: deps.api.addr_make("withdraw_manager_contract"),
            asset: "asset".to_string(),
        },
    };
    let info = message_info(&owner, &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(matches!(
        err,
        ContractError::Std(StdError::GenericErr { .. })
    ));
}

#[test]
fn test_instantiate_with_invalid_contract() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");

    let msg = InstantiateMsg {
        owner: owner.to_string(),
        config: Config {
            locker: deps.api.addr_make("locker"),
            unlocker: deps.api.addr_make("unlocker"),
            withdraw_manager_contract: Addr::unchecked("invalid...address..."),
            asset: "asset".to_string(),
        },
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
        config: Config {
            locker: deps.api.addr_make("locker"),
            unlocker: deps.api.addr_make("unlocker"),
            withdraw_manager_contract: deps.api.addr_make("withdraw_manager_contract"),
            asset: "asset".to_string(),
        },
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
    assert_eq!(result, ContractError::Ownable(NotOwner));

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
    assert_eq!(result, ContractError::Ownable(NotPendingOwner));

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
        new_config: UpdateConfig {
            locker: Some(deps.api.addr_make("new_locker").to_string()),
            unlocker: Some(deps.api.addr_make("new_unlocker").to_string()),
            withdraw_manager_contract: Some(
                deps.api
                    .addr_make("new_withdraw_manager_contract")
                    .to_string(),
            ),
            asset: Some("new_asset".to_string()),
        },
    };

    let res = execute_msg(&mut deps, mock_env(), &owner, msg).unwrap();
    assert_eq!(0, res.messages.len());

    let config = CONFIG.load(&deps.storage).unwrap();
    let expected_config = Config {
        locker: deps.api.addr_make("new_locker"),
        unlocker: deps.api.addr_make("new_unlocker"),
        withdraw_manager_contract: deps.api.addr_make("new_withdraw_manager_contract"),
        asset: "new_asset".to_string(),
    };
    assert_config_equals(&config, &expected_config);
}

#[test]
fn update_config_by_unauthorized() {
    let (mut deps, _env) = setup_contract();
    let oracle1 = deps.api.addr_make("oracle1");

    let msg = ExecuteMsg::UpdateConfig {
        new_config: UpdateConfig {
            locker: Some(deps.api.addr_make("new_locker").to_string()),
            unlocker: Some(deps.api.addr_make("new_unlocker").to_string()),
            withdraw_manager_contract: Some(
                deps.api
                    .addr_make("new_withdraw_manager_contract")
                    .to_string(),
            ),
            asset: Some("new_asset".to_string()),
        },
    };

    let err = execute_msg(&mut deps, mock_env(), &oracle1, msg).unwrap_err();
    assert_eq!(err, ContractError::Ownable(NotOwner));

    let config = CONFIG.load(&deps.storage).unwrap();
    let expected_config = Config {
        locker: deps.api.addr_make("locker"),
        unlocker: deps.api.addr_make("unlocker"),
        withdraw_manager_contract: deps.api.addr_make("withdraw_manager_contract"),
        asset: "asset".to_string(),
    };
    assert_config_equals(&config, &expected_config);
}

#[test]
fn query_config() {
    let (deps, _env) = setup_contract();
    let bin = query_msg(&deps, mock_env(), QueryMsg::GetConfig {}).unwrap();
    let config: Config = from_json(bin).unwrap();

    let expected_config = Config {
        locker: deps.api.addr_make("locker"),
        unlocker: deps.api.addr_make("unlocker"),
        withdraw_manager_contract: deps.api.addr_make("withdraw_manager_contract"),
        asset: "asset".to_string(),
    };
    assert_config_equals(&config, &expected_config);
}

#[test]
fn query_default_state() {
    let (deps, _env) = setup_contract();
    let bin = query_msg(&deps, mock_env(), QueryMsg::GetState {}).unwrap();
    let res: State = from_json(bin).unwrap();

    assert_eq!(res, State::Unlocked {});
}

#[test]
fn lock_and_query_state() {
    let (mut deps, _env) = setup_contract();
    let locker = deps.api.addr_make("locker");

    let lock_msg = ExecuteMsg::Lock {
        amount: Uint128::new(100_500_000u128),
    };
    let res = execute_msg(&mut deps, mock_env(), &locker, lock_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let bin = query_msg(&deps, mock_env(), QueryMsg::GetState {}).unwrap();
    let res: State = from_json(bin).unwrap();

    assert_eq!(
        res,
        State::Locked {
            amount: Uint128::new(100_500_000u128),
            at_timestamp: mock_env().block.time.nanos(),
        }
    );
}
#[test]
fn lock_already_locked() {
    let (mut deps, _env) = setup_contract();
    let locker = deps.api.addr_make("locker");

    let lock_msg = ExecuteMsg::Lock {
        amount: Uint128::new(100_500_000u128),
    };
    let res = execute_msg(&mut deps, mock_env(), &locker, lock_msg.clone()).unwrap();
    assert_eq!(0, res.messages.len());

    let err = execute_msg(&mut deps, mock_env(), &locker, lock_msg).unwrap_err();
    assert_eq!(err, ContractError::AlreadyLocked {});
}

#[test]
fn lock_unauthorized() {
    let (mut deps, _env) = setup_contract();
    let stranger = deps.api.addr_make("stranger");

    let lock_msg = ExecuteMsg::Lock {
        amount: Uint128::new(100_500_000u128),
    };
    let err = execute_msg(&mut deps, mock_env(), &stranger, lock_msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn unlock_not_locked() {
    let (mut deps, _env) = setup_contract();
    let stranger = deps.api.addr_make("unlocker");

    let unlock_msg = ExecuteMsg::Unlock {};
    let err = execute_msg(&mut deps, mock_env(), &stranger, unlock_msg).unwrap_err();
    assert_eq!(err, ContractError::AlreadyUnlocked {});
}

#[test]
fn unlock_unauthorized() {
    let (mut deps, _env) = setup_contract();
    let stranger = deps.api.addr_make("stranger");

    let unlock_msg = ExecuteMsg::Unlock {};
    let err = execute_msg(&mut deps, mock_env(), &stranger, unlock_msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]

fn unlock_success() {
    let (mut deps, env) = setup_contract();
    let locker = deps.api.addr_make("locker");
    let unlocker = deps.api.addr_make("unlocker");

    deps.querier.set_balance(
        env.contract.address.as_ref(),
        "asset",
        Uint128::new(100_500_000u128),
    );

    let lock_msg = ExecuteMsg::Lock {
        amount: Uint128::new(100_500_000u128),
    };
    let res = execute_msg(&mut deps, mock_env(), &locker, lock_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let unlock_msg = ExecuteMsg::Unlock {};
    let res = execute_msg(&mut deps, mock_env(), &unlocker, unlock_msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: deps.api.addr_make("withdraw_manager_contract").to_string(),
            amount: vec![Coin {
                denom: "asset".to_string(),
                amount: Uint128::new(100_500_000u128),
            }],
        })),]
    );

    let bin = query_msg(&deps, mock_env(), QueryMsg::GetState {}).unwrap();
    let res: State = from_json(bin).unwrap();

    assert_eq!(res, State::Unlocked {});
}

#[test]
fn unlock_no_data() {
    let (mut deps, env) = setup_contract();
    let locker = deps.api.addr_make("locker");
    let unlocker = deps.api.addr_make("unlocker");

    deps.querier
        .set_balance(env.contract.address.as_ref(), "asset", Uint128::zero());

    let lock_msg = ExecuteMsg::Lock {
        amount: Uint128::new(100_500_000u128),
    };
    let res = execute_msg(&mut deps, mock_env(), &locker, lock_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let bin = query_msg(&deps, mock_env(), QueryMsg::GetState {}).unwrap();
    let res: State = from_json(bin).unwrap();
    assert_eq!(
        res,
        State::Locked {
            amount: Uint128::new(100_500_000u128),
            at_timestamp: mock_env().block.time.nanos(),
        }
    );

    let unlock_msg = ExecuteMsg::Unlock {};
    let res = execute_msg(&mut deps, mock_env(), &unlocker, unlock_msg).unwrap_err();
    assert_eq!("Insufficient asset amount to unlock", res.to_string());

    let bin = query_msg(&deps, mock_env(), QueryMsg::GetState {}).unwrap();
    let res: State = from_json(bin).unwrap();

    assert_eq!(
        res,
        State::Locked {
            amount: Uint128::new(100_500_000u128),
            at_timestamp: mock_env().block.time.nanos(),
        }
    );
}

#[test]
fn unlock_little_amount() {
    let (mut deps, _env) = setup_contract();
    let locker = deps.api.addr_make("locker");
    let unlocker = deps.api.addr_make("unlocker");

    let lock_msg = ExecuteMsg::Lock {
        amount: Uint128::new(5_025_000u128),
    };
    let res = execute_msg(&mut deps, mock_env(), &locker, lock_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let bin = query_msg(&deps, mock_env(), QueryMsg::GetState {}).unwrap();
    let res: State = from_json(bin).unwrap();
    assert_eq!(
        res,
        State::Locked {
            amount: Uint128::new(5_025_000u128),
            at_timestamp: mock_env().block.time.nanos(),
        }
    );

    let unlock_msg = ExecuteMsg::Unlock {};
    let res = execute_msg(&mut deps, mock_env(), &unlocker, unlock_msg).unwrap_err();
    assert_eq!("Insufficient asset amount to unlock", res.to_string());

    let bin = query_msg(&deps, mock_env(), QueryMsg::GetState {}).unwrap();
    let res: State = from_json(bin).unwrap();

    assert_eq!(
        res,
        State::Locked {
            amount: Uint128::new(5_025_000u128),
            at_timestamp: mock_env().block.time.nanos(),
        }
    );
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
        config: Config {
            locker: deps.api.addr_make("locker"),
            unlocker: deps.api.addr_make("unlocker"),
            withdraw_manager_contract: deps.api.addr_make("withdraw_manager_contract"),
            asset: "asset".to_string(),
        },
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
) -> Result<cosmwasm_std::Response, ContractError>
where
    T: cosmwasm_std::Querier,
{
    let info = message_info(sender, &[]);
    execute(deps.as_mut(), env, info, msg)
}

/// Query helper
fn query_msg<T>(
    deps: &OwnedDeps<MockStorage, MockApi, T>,
    env: Env,
    msg: QueryMsg,
) -> Result<cosmwasm_std::Binary, ContractError>
where
    T: cosmwasm_std::Querier,
{
    query(deps.as_ref(), env, msg)
}

/// Config verification helper
fn assert_config_equals(config: &Config, expected_config: &Config) {
    assert_eq!(config.locker, expected_config.locker);
    assert_eq!(config.unlocker, expected_config.unlocker);
    assert_eq!(config.asset, expected_config.asset);
}
