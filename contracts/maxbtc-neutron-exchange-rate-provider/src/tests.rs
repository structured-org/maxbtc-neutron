use crate::{
    contract::{execute, instantiate, query},
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
};
use cosmwasm_std::{
    from_json,
    testing::{message_info, mock_dependencies, mock_env},
    Addr, Decimal,
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
fn test_update_exchange_rate() {
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
    let execute_msg = ExecuteMsg::UpdateExchangeRate {
        rate: Decimal::percent(99),
    };
    let res = execute(deps.as_mut(), env.clone(), info, execute_msg).unwrap();
    assert_eq!(res.attributes.len(), 2);
    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "update_exchange_rate");
    assert_eq!(res.attributes[1].key, "rate");
    assert_eq!(res.attributes[1].value, "0.99");

    // Query the exchange rate
    let query_msg = QueryMsg::ExchangeRate {};
    let res = query(deps.as_ref(), env, query_msg).unwrap();
    let rate: Decimal = from_json(&res).unwrap();
    assert_eq!(rate, Decimal::percent(99));
}
