use crate::{
    contract::{execute, instantiate, query},
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
};
use cosmwasm_std::{
    from_json,
    testing::{message_info, mock_dependencies, mock_env},
    Addr,
};

use crate::contract::migrate;
use crate::msg::MigrateMsg;
use crate::state::{ALLOW_LIST, ALLOW_LIST_V1};
use cw2::{get_contract_version, set_contract_version};

#[test]
fn test_update_ownership() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(
        &Addr::unchecked("cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh"),
        &[],
    );

    // Instantiate the contract
    let msg = InstantiateMsg {
        owner: "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
    };
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Update ownership
    let action = cw_ownable::Action::TransferOwnership {
        new_owner: "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string(),
        expiry: None,
    };
    let execute_msg = ExecuteMsg::UpdateOwnership(action);
    execute(deps.as_mut(), env.clone(), info, execute_msg).unwrap();

    // accept ownership transfer
    let info = message_info(
        &Addr::unchecked("cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55"),
        &[],
    );
    let action = cw_ownable::Action::AcceptOwnership {};
    let execute_msg = ExecuteMsg::UpdateOwnership(action);
    let res = execute(deps.as_mut(), env, info, execute_msg).unwrap();

    assert_eq!(res.attributes.len(), 1);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_ownership");
}

#[test]
fn test_allow_addresses() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(
        &Addr::unchecked("cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh"),
        &[],
    );
    // Instantiate the contract
    let msg = InstantiateMsg {
        owner: "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
    };
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    // Update the allow list
    let execute_msg = ExecuteMsg::Allow {
        addresses: vec![
            "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string(),
            "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
        ],
    };
    let res = execute(deps.as_mut(), env.clone(), info, execute_msg).unwrap();
    assert_eq!(res.attributes.len(), 3);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "allow_addresses");
    assert_eq!(res.attributes[1].key, "allowed_address");
    assert_eq!(
        res.attributes[1].value,
        "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55"
    );
    assert_eq!(res.attributes[2].key, "allowed_address");
    assert_eq!(
        res.attributes[2].value,
        "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh"
    );

    // Query the allow list
    let query_msg = QueryMsg::AllowList {
        limit: Some(10u32),
        start_after: None,
    };
    let res = query(deps.as_ref(), env, query_msg).unwrap();
    let allow_list: Vec<String> = from_json(&res).unwrap();
    assert_eq!(allow_list.len(), 2);
    assert!(allow_list.contains(&"cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string()));
    assert!(allow_list.contains(&"cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string()));
}

#[test]
fn test_allow_list_no_admin() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(
        &Addr::unchecked("cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh"),
        &[],
    );
    // Instantiate the contract
    let msg = InstantiateMsg {
        owner: "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
    };
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    // Update the allow list
    let info = message_info(
        &Addr::unchecked("cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55"),
        &[],
    );
    let execute_msg = ExecuteMsg::Allow {
        addresses: vec![
            "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string(),
            "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
        ],
    };
    let res = execute(deps.as_mut(), env.clone(), info, execute_msg);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Caller is not the contract's current owner"
    );
}

#[test]
fn test_deny_addresses() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(
        &Addr::unchecked("cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh"),
        &[],
    );
    // Instantiate the contract
    let msg = InstantiateMsg {
        owner: "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
    };
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    // Update the allow list
    let execute_msg = ExecuteMsg::Allow {
        addresses: vec![
            "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string(),
            "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
        ],
    };
    execute(deps.as_mut(), env.clone(), info.clone(), execute_msg).unwrap();

    let execute_msg = ExecuteMsg::Deny {
        addresses: vec!["cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string()],
    };
    let res = execute(deps.as_mut(), env.clone(), info, execute_msg).unwrap();

    assert_eq!(res.attributes.len(), 2);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "deny_addresses");
    assert_eq!(res.attributes[1].key, "denied_address");
    assert_eq!(
        res.attributes[1].value,
        "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55"
    );

    // Query the allow list
    let query_msg = QueryMsg::AllowList {
        limit: Some(10u32),
        start_after: None,
    };
    let res = query(deps.as_ref(), env, query_msg).unwrap();
    let allow_list: Vec<String> = from_json(&res).unwrap();
    assert_eq!(allow_list.len(), 1);
    assert!(allow_list.contains(&"cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string()));
}

#[test]
fn test_deny_list_no_admin() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(
        &Addr::unchecked("cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh"),
        &[],
    );
    // Instantiate the contract
    let msg = InstantiateMsg {
        owner: "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
    };
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    // Update the allow list
    let info = message_info(
        &Addr::unchecked("cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55"),
        &[],
    );
    let execute_msg = ExecuteMsg::Deny {
        addresses: vec![
            "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string(),
            "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
        ],
    };
    let res = execute(deps.as_mut(), env.clone(), info, execute_msg);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Caller is not the contract's current owner"
    );
}

#[test]
fn test_is_address_allowed_by_allow_list() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(
        &Addr::unchecked("cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh"),
        &[],
    );
    // Instantiate the contract
    let msg = InstantiateMsg {
        owner: "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
    };
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    // Update the allow list
    let execute_msg = ExecuteMsg::Allow {
        addresses: vec![
            "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string(),
            "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
        ],
    };
    execute(deps.as_mut(), env.clone(), info, execute_msg).unwrap();
    // Check if an address is allowed
    let query_msg = QueryMsg::IsAddressAllowed {
        address: "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string(),
    };
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let is_allowed: bool = from_json(&res).unwrap();
    assert!(is_allowed);
    // Check if an address is not allowed
    let query_msg = QueryMsg::IsAddressAllowed {
        address: "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql3pzxaxu".to_string(), //not exists
    };
    let res = query(deps.as_ref(), env, query_msg).unwrap();
    let is_allowed: bool = from_json(&res).unwrap();
    assert!(!is_allowed);
}

#[test]
fn test_migrate_from_v1_to_v2() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    // Simulate old contract version and v1 allow list
    set_contract_version(
        deps.as_mut().storage,
        "crates.io:maxbtc-neutron-allow-list",
        "0.2.0",
    )
    .unwrap();

    let old_addresses = vec![Addr::unchecked("cosmwasm1a"), Addr::unchecked("cosmwasm1b")];
    ALLOW_LIST_V1
        .save(deps.as_mut().storage, &old_addresses)
        .unwrap();

    // Migrate
    let res = migrate(deps.as_mut(), env.clone(), MigrateMsg {}).unwrap();
    assert_eq!(res.attributes.len(), 0);

    // Check contract version updated
    let version = get_contract_version(deps.as_ref().storage).unwrap();
    assert_eq!(version.contract, "crates.io:maxbtc-neutron-allow-list");
    // Version should not be 0.2.0 anymore
    assert_ne!(version.version, "0.2.0");

    // Check addresses migrated to v2
    for addr in old_addresses {
        let allowed = ALLOW_LIST.may_load(deps.as_ref().storage, &addr).unwrap();
        assert!(allowed.is_some());
    }
}
