use std::marker::PhantomData;

use crate::contract::{execute, get_maxbtc_denom, instantiate};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::state::CONFIG;
use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    Api, Attribute, DepsMut, Empty, Env, MessageInfo, OwnedDeps, Response, Uint128,
};

pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, MockQuerier, Empty> {
    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: MockQuerier::default(),
        custom_query_type: PhantomData,
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
    // We expect 2 message: instantiate2 msg for the fee collector, and create_tokenfactory_create_denom_msg
    assert_eq!(res.messages.len(), 1);
    // Assert: check attributes
    let expected_attributes = vec![
        Attribute::new("action", "instantiate"),
        Attribute::new("owner", msg.owner.clone()),
        Attribute::new("factory_contract", msg.factory_contract.clone()),
        Attribute::new(
            "denom",
            get_maxbtc_denom(env.contract.address.to_string(), msg.subdenom.clone()),
        ),
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

    // etc. check more fields
    assert_eq!(
        cfg.factory_contract,
        deps.api
            .addr_validate(msg.factory_contract.as_str())
            .unwrap()
    );
    assert_eq!(
        cfg.denom,
        get_maxbtc_denom(env.contract.address.to_string(), msg.subdenom.clone())
    );

    // Finally, check time-based items
    // let last_deposit_flush_time = LAST_DEPOSIT_FLUSH_TIME.load(&deps.storage).unwrap();
    // assert_eq!(last_deposit_flush_time, env.block.time.seconds());
}

#[test]
fn test_mint_success() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let info = message_info(&deps.api.addr_make("owner_addr"), &[]);

    // Act
    let mint_amount: Uint128 = Uint128::from(1_000_000u128);
    let recipient = deps.api.addr_make("recipient_addr");
    let res = do_mint(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        recipient.to_string(),
        mint_amount,
    )
    .unwrap();

    // Assert
    // The response should have exactly one message: the tokenfactory Mint
    assert_eq!(res.messages.len(), 1);

    let minted_attr = res
        .attributes
        .iter()
        .find(|attr| attr.key == "amount")
        .expect("amount attribute must be present");
    assert_eq!(minted_attr.value, "1000000");
}

#[test]
fn test_mint_zero_amount() {
    let (mut deps, env, _) = setup_contract();

    let info = message_info(&deps.api.addr_make("owner_addr"), &[]);

    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_mint(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        recipient,
        Uint128::zero(),
    )
    .expect_err("Zero deposit is invalid");

    match err {
        ContractError::InvalidDepositAmount {} => (),
        e => panic!("Unexpected error: {:?}", e),
    }
}

/// -----------------------------------------------------------------------------------------------
/// HELPER FUNCTIONS BELOW
/// -----------------------------------------------------------------------------------------------

/// Initializes the contract and sets up a "happy path" config in storage.
/// Returns a mutable Deps and an Env, Info you can reuse in tests.
fn setup_contract() -> (
    OwnedDeps<MockStorage, MockApi, MockQuerier>,
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

fn default_instantiate_msg(deps: &OwnedDeps<MockStorage, MockApi, MockQuerier>) -> InstantiateMsg {
    InstantiateMsg {
        owner: deps.api.addr_make("owner_addr").to_string(),
        factory_contract: deps.api.addr_make("factory_addr").to_string(),
        subdenom: "maxbtc".to_string(),
    }
}

/// A convenience helper for calling the `execute_mint` entry point.
fn do_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    execute(
        deps,
        env,
        info,
        ExecuteMsg::Mint {
            recipient: recipient.to_string(),
            amount,
        },
    )
}
