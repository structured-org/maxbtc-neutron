use crate::{
    contract::{execute, instantiate, query},
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
};
use cosmwasm_std::{
    from_json,
    testing::{message_info, mock_dependencies, mock_env},
    Addr,
};

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

    assert_eq!(res.attributes.len(), 2);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_ownership");
    assert_eq!(res.attributes[1].key, "new_owner");
    assert_eq!(
        res.attributes[1].value,
        "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55"
    );
}

#[test]
fn test_update_allow_list() {
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
    let execute_msg = ExecuteMsg::UpdateAllowList {
        allow_list: vec![
            "cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string(),
            "cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string(),
        ],
    };
    let res = execute(deps.as_mut(), env.clone(), info, execute_msg).unwrap();
    assert_eq!(res.attributes.len(), 2);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_allow_list");
    assert_eq!(res.attributes[1].key, "allow_list");
    assert_eq!(
        res.attributes[1].value,
         "[Addr(\"cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55\"), Addr(\"cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh\")]"
    );
    // Query the allow list
    let query_msg = QueryMsg::AllowList {};
    let res = query(deps.as_ref(), env, query_msg).unwrap();
    let allow_list: Vec<String> = from_json(&res).unwrap();
    assert_eq!(allow_list.len(), 2);
    assert!(allow_list.contains(&"cosmwasm1ygejj7rnheqlvvmcnmggllcd9y226ql5n7sw55".to_string()));
    assert!(allow_list.contains(&"cosmwasm1jy7lsk5pk38zjfnn6nt6qlaphy9uejn496zwvh".to_string()));
}

#[test]
fn test_update_allow_list_no_admin() {
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
    let execute_msg = ExecuteMsg::UpdateAllowList {
        allow_list: vec![
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
    let execute_msg = ExecuteMsg::UpdateAllowList {
        allow_list: vec![
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
        address: "cosmwasm1nonexistentaddress".to_string(),
    };
    let res = query(deps.as_ref(), env, query_msg).unwrap();
    let is_allowed: bool = from_json(&res).unwrap();
    assert!(!is_allowed);
}
