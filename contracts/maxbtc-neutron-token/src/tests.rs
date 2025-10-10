use std::marker::PhantomData;

use crate::contract::{execute, get_maxbtc_denom, instantiate};
use crate::error::ContractError;
use crate::msg::{DenomMetadata, ExecuteMsg, InstantiateMsg};
use crate::state::CONFIG;
use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    coin, Api, Attribute, DepsMut, Empty, Env, MessageInfo, OwnedDeps, Response, Uint128,
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

#[test]
fn test_mint_wrong_owner() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let info = message_info(&deps.api.addr_make("not_owner_addr"), &[]);

    // Act
    let recipient = deps.api.addr_make("recipient_addr").to_string();
    let err = do_mint(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        recipient,
        Uint128::zero(),
    )
    .unwrap_err();

    // Assert
    match err {
        ContractError::OwnershipError(cw_ownable::OwnershipError::NotOwner) => {}
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_burn_success() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let burn_amount: Uint128 = Uint128::from(1_000_000u128);

    let info = message_info(
        &deps.api.addr_make("owner_addr"),
        &[coin(
            burn_amount.u128(),
            "factory/cosmwasm1jpev2csrppg792t22rn8z8uew8h3sjcpglcd0qv9g8gj8ky922tscp8avs/maxbtc",
        )],
    );

    // Act
    let res = do_burn(deps.as_mut(), env.clone(), info.clone()).unwrap();

    // Assert
    // The response should have exactly one message: the tokenfactory Mint
    assert_eq!(res.messages.len(), 1);

    let burned_attr = res
        .attributes
        .iter()
        .find(|attr| attr.key == "amount")
        .expect("amount attribute must be present");
    assert_eq!(burned_attr.value, "1000000");
}

#[test]
fn test_burn_wrong_owner_allowed() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let burn_amount: Uint128 = Uint128::from(1_000_000u128);
    let info = message_info(
        &deps.api.addr_make("not_owner_addr"),
        &[coin(
            burn_amount.u128(),
            "factory/cosmwasm1jpev2csrppg792t22rn8z8uew8h3sjcpglcd0qv9g8gj8ky922tscp8avs/maxbtc",
        )],
    );

    // Act
    let res = do_burn(deps.as_mut(), env.clone(), info.clone());

    assert!(res.is_ok());
}

#[test]
fn test_burn_wrong_denom() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let burn_amount: Uint128 = Uint128::from(1_000_000u128);

    let info = message_info(
        &deps.api.addr_make("owner_addr"),
        &[coin(burn_amount.u128(), "untrn")],
    );

    // Act
    let err =
        do_burn(deps.as_mut(), env.clone(), info.clone()).expect_err("Zero deposit is invalid");

    match err {
        ContractError::PaymentError(cw_utils::PaymentError::MissingDenom(_)) => {}
        e => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_set_token_metadata_success() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let info = message_info(&deps.api.addr_make("owner_addr"), &[]);

    // Act
    let res = do_set_token_metadata(deps.as_mut(), env.clone(), info.clone()).unwrap();

    // Assert
    // The response should have exactly one message: the tokenfactory Mint
    assert_eq!(res.messages.len(), 1);

    assert_eq!(
        res.attributes,
        vec![
            Attribute::new("action", "set_token_metadata"),
            Attribute::new("sender", info.sender.to_string()),
            Attribute::new("denom", "factory/cosmwasm1jpev2csrppg792t22rn8z8uew8h3sjcpglcd0qv9g8gj8ky922tscp8avs/maxbtc"),
            Attribute::new("exponent", "0"),
            Attribute::new("display", "maxBTC"),
            Attribute::new("name", "maxBTC"),
            Attribute::new("description", "maxBTC"),
            Attribute::new("symbol", "maxBTC"),
            Attribute::new("uri", ""),
            Attribute::new("uri_hash", ""),
        ]
    );
}

#[test]
fn test_set_token_metadata_wrong_owner() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let info = message_info(&deps.api.addr_make("not_owner_addr"), &[]);

    // Act
    let err = do_set_token_metadata(deps.as_mut(), env.clone(), info.clone()).unwrap_err();

    // Assert
    match err {
        ContractError::OwnershipError(cw_ownable::OwnershipError::NotOwner) => {}
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

/// A convenience helper for calling the `execute_burn` entry point.
fn do_burn(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    execute(deps, env, info, ExecuteMsg::Burn {})
}

/// A convenience helper for calling the `execute_set_token_metadata` entry point.
fn do_set_token_metadata(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    execute(
        deps,
        env,
        info,
        ExecuteMsg::SetTokenMetadata {
            token_metadata: DenomMetadata {
                name: "maxBTC".to_string(),
                symbol: "maxBTC".to_string(),
                display: "maxBTC".to_string(),
                uri: None,
                uri_hash: None,
                description: "maxBTC".to_string(),
                exponent: 0,
            },
        },
    )
}
