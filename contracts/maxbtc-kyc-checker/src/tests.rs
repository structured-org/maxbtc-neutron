use crate::contract::{instantiate, query, query_has_approved};
use crate::msgs::{InstantiateMsg, QueryMsg};
use crate::state::{Config, ZkmeVerifyQueryMsg, CONFIG};
use cosmwasm_std::testing::{mock_dependencies, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    attr, from_json,
    testing::{message_info, mock_env},
    Addr, Event, Response,
};
use cosmwasm_std::{
    to_json_binary, ContractResult, OwnedDeps, SystemError, SystemResult, WasmQuery,
};
use cw_ownable::Ownership;

#[test]
fn test_instantiate_general() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let sender = deps.api.addr_make("sender");
    let zkme_verify_upgradeable_contract = deps.api.addr_make("zkme_verify_upgradeable_contract");
    let cooperator_address = "cooperator_address";

    let deps_mut = deps.as_mut();
    let response = instantiate(
        deps_mut,
        mock_env(),
        message_info(&sender, &[]),
        InstantiateMsg {
            owner: owner.to_string(),
            zkme_verify_upgradeable_contract: zkme_verify_upgradeable_contract.to_string(),
            cooperator_address: cooperator_address.to_string(),
        },
    )
    .unwrap();

    assert_eq!(
        response,
        Response::new().add_event(
            Event::new("crates.io:maxbtc-kyc-checker-instantiate").add_attributes(vec![
                attr(
                    "zkme_verify_upgradeable_contract",
                    zkme_verify_upgradeable_contract.to_string()
                ),
                attr("cooperator_address", cooperator_address),
                attr("owner", owner.to_string()),
            ])
        )
    );

    let res: Config = from_json(
        query(
            deps.as_ref().into_empty(),
            mock_env(),
            QueryMsg::GetConfig {},
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        res,
        Config {
            zkme_verify_upgradeable_contract: Addr::unchecked(zkme_verify_upgradeable_contract),
            cooperator_address: cooperator_address.to_string(),
        }
    );
}

#[test]
fn test_query_owner() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");

    let deps_mut = deps.as_mut();
    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some(owner.as_ref())).unwrap();

    let owner_data: Ownership<Addr> = from_json(
        query(
            deps.as_ref().into_empty(),
            mock_env(),
            QueryMsg::Ownership {},
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(owner_data.owner, Some(owner));
}

#[test]
fn test_query_has_approved() {
    let zkme_verify_upgradeable_contract = "zkme_verify_upgradeable_contract";
    let cooperator = "cooperator";
    let user = "user";

    let mut querier: MockQuerier<cosmwasm_std::Empty> = MockQuerier::new(&[]);
    querier.update_wasm(move |query| match query {
        WasmQuery::Smart { contract_addr, msg } => {
            if zkme_verify_upgradeable_contract == contract_addr {
                let requested: ZkmeVerifyQueryMsg = from_json(msg).unwrap();
                assert_eq!(
                    requested,
                    ZkmeVerifyQueryMsg::HasApproved {
                        cooperator: cooperator.to_string(),
                        user: user.to_string()
                    }
                );
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&true).unwrap()))
            } else {
                SystemResult::Err(SystemError::NoSuchContract {
                    addr: zkme_verify_upgradeable_contract.into(),
                })
            }
        }
        _ => panic!("Unexpected query"),
    });

    let mut deps: OwnedDeps<_, _, MockQuerier, cosmwasm_std::Empty> = OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier,
        custom_query_type: std::marker::PhantomData,
    };
    let owner = deps.api.addr_make("owner");

    let deps_mut = deps.as_mut();

    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some(owner.as_ref())).unwrap();

    CONFIG
        .save(
            deps_mut.storage,
            &Config {
                zkme_verify_upgradeable_contract: Addr::unchecked(zkme_verify_upgradeable_contract),
                cooperator_address: cooperator.to_string(),
            },
        )
        .unwrap();

    let mocked_env = mock_env();

    let result =
        query_has_approved(deps_mut.as_ref(), mocked_env.clone(), user.to_string()).unwrap();

    let has_approved: bool = from_json(result).unwrap();

    assert!(has_approved);
}

#[test]
fn test_query_not_approved() {
    let zkme_verify_upgradeable_contract = "zkme_verify_upgradeable_contract";
    let cooperator = "cooperator";
    let user = "user";

    let mut querier: MockQuerier<cosmwasm_std::Empty> = MockQuerier::new(&[]);
    querier.update_wasm(move |query| match query {
        WasmQuery::Smart {
            contract_addr: _,
            msg: _,
        } => SystemResult::Ok(ContractResult::Ok(to_json_binary(&false).unwrap())),
        _ => panic!("Unexpected query"),
    });

    let mut deps: OwnedDeps<_, _, MockQuerier, cosmwasm_std::Empty> = OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier,
        custom_query_type: std::marker::PhantomData,
    };
    let owner = deps.api.addr_make("owner");

    let deps_mut = deps.as_mut();

    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some(owner.as_ref())).unwrap();

    CONFIG
        .save(
            deps_mut.storage,
            &Config {
                zkme_verify_upgradeable_contract: Addr::unchecked(zkme_verify_upgradeable_contract),
                cooperator_address: cooperator.to_string(),
            },
        )
        .unwrap();

    let mocked_env = mock_env();

    let result =
        query_has_approved(deps_mut.as_ref(), mocked_env.clone(), user.to_string()).unwrap();

    let has_approved: bool = from_json(result).unwrap();

    assert!(!has_approved);
}
