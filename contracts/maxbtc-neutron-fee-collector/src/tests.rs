use std::collections::HashMap;
use std::marker::PhantomData;
use std::str::FromStr;

use crate::contract::{calculate_fee_to_mint, execute, instantiate};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::state::CONFIG;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    coin, from_json, to_json_binary, Attribute, BankQuery, Binary, Coin, ContractResult, CosmosMsg,
    Decimal, DepsMut, Empty, Env, MessageInfo, OwnedDeps, Querier, QuerierResult, QueryRequest,
    Response, SystemError, SystemResult, Uint128, WasmMsg, WasmQuery,
};
use maxbtc_base::msg::core::{ExecuteMsg as CoreExecuteMsg, QueryMsg as CoreQueryMsg};

#[cw_serde]
#[non_exhaustive]
pub struct SupplyResponse {
    /// Always returns a Coin with the requested denom.
    /// This will be of zero amount if the denom does not exist.
    pub amount: Coin,
}

/// A custom mock querier that can handle WASM queries.
pub struct WasmMockQuerier {
    base: MockQuerier<Empty>,
    supplies: HashMap<String, Uint128>,
    exchange_rate: Decimal,
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<Empty>) -> Self {
        WasmMockQuerier {
            base,
            supplies: HashMap::new(),
            exchange_rate: Decimal::one(),
        }
    }

    pub fn set_supply(&mut self, denom: &str, amount: Uint128) {
        self.supplies.insert(denom.into(), amount);
    }

    pub fn set_exchange_rate(&mut self, exchange_rate: Decimal) {
        self.exchange_rate = exchange_rate;
    }

    fn handle_bank_query(&self, query: &BankQuery) -> QuerierResult {
        match query {
            BankQuery::Supply { denom } => {
                if let Some(val) = self.supplies.get(denom) {
                    let resp = SupplyResponse {
                        amount: coin(val.u128(), denom.clone()),
                    };
                    return SystemResult::Ok(ContractResult::Ok(to_json_binary(&resp).unwrap()));
                }

                self.base
                    .handle_query(&QueryRequest::Bank(BankQuery::Supply {
                        denom: denom.clone(),
                    }))
            }
            // For other queries, fallback to base
            _ => self.base.handle_query(&QueryRequest::Bank(query.clone())),
        }
    }

    fn handle_wasm_query(&self, wasm_query: &WasmQuery) -> SystemResult<ContractResult<Binary>> {
        match wasm_query {
            WasmQuery::Smart { contract_addr, msg } => {
                // Attempt to decode into one of our known query message types
                self.handle_wasm_smart_query(contract_addr, msg)
            }
            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                kind: "Unsupported WASM query".to_string(),
            }),
        }
    }

    /// Dispatches our recognized wasm queries to the correct mock data.
    fn handle_wasm_smart_query(&self, contract_addr: &str, msg: &Binary) -> QuerierResult {
        println!("Handling wasm query for contract: {contract_addr}");

        // Core contract query
        if contract_addr == "cosmwasm1srehv4hmnw0usukp5snwvatkwf2fss9ewx6lzgx2egq9re8px22shd2apt" {
            let parsed: Result<CoreQueryMsg, _> = from_json(msg);
            if let Ok(q) = parsed {
                return match q {
                    CoreQueryMsg::ExchangeRate {} => {
                        let val = self.exchange_rate;
                        SystemResult::Ok(ContractResult::Ok(to_json_binary(&val).unwrap()))
                    }
                    _ => {
                        unimplemented!()
                    }
                };
            }
            // If parse failed or unsupported => fallback
            return self
                .base
                .handle_query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: contract_addr.into(),
                    msg: msg.clone(),
                }));
        }

        // If it's not one of our recognized addresses, fallback to base
        self.base
            .handle_query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: contract_addr.into(),
                msg: msg.clone(),
            }))
    }

    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Bank(bank_query) => self.handle_bank_query(bank_query),
            QueryRequest::Wasm(wasm_query) => self.handle_wasm_query(wasm_query),
            _ => self.base.handle_query(request),
        }
    }
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<Empty> = match from_json(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return QuerierResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {e}"),
                    request: bin_request.into(),
                });
            }
        };
        self.handle_query(&request)
    }
}

pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier = WasmMockQuerier::new(MockQuerier::new(&[]));

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
        custom_query_type: PhantomData,
    }
}

#[test]
fn test_calculate_fee_to_mint() {
    // Case 1: Standard positive APY
    // Old rate: 1.0, Current rate: 1.1. Gain is 0.1.
    // Total supply: 1,000,000
    // Decimals: 6
    // Fee reduction: 10% (0.1)
    // We want to skim 10% of the 0.1 gain.
    // Retained gain should be 0.09. Target rate = 1.0 + 0.09 = 1.09.
    // total_supply_dec = 1.0
    // fee_dec = 1.0 * (1.1 / 1.09 - 1)
    // fee_dec = 1.0 * (0.0091743119...)
    // fee_dec = 0.0091743119...
    // Converting back to atomics (6 decimals) should give 9174
    let rate_old = Decimal::from_str("1.0").unwrap();
    let rate_current = Decimal::from_str("1.1").unwrap();
    let total_supply = Uint128::new(1_000_000); // 1.0 with 6 decimals
    let fee_percentage = Decimal::from_str("0.1").unwrap(); // 10%
    let decimals = 6;

    let fee = calculate_fee_to_mint(
        rate_old,
        rate_current,
        total_supply,
        fee_percentage,
        decimals,
    )
    .unwrap();
    assert_eq!(fee, Uint128::new(9174));

    // New total supply = 1,000,000 + 9174 = 1,009,174
    // Total assets (in value) = rate_current * old_supply_dec = 1.1 * 1.0 = 1.1
    // New rate = Total assets / new_supply_dec = 1.1 / 1.009174 = 1.0900002...
    // This is very close to the target of 1.09.

    // Case 2: No gain
    let rate_old_2 = Decimal::from_str("1.1").unwrap();
    let rate_current_2 = Decimal::from_str("1.1").unwrap();
    let fee_2 = calculate_fee_to_mint(
        rate_old_2,
        rate_current_2,
        total_supply,
        fee_percentage,
        decimals,
    )
    .unwrap();
    assert_eq!(fee_2, Uint128::zero());

    // Case 3: Negative APY (loss)
    let rate_old_3 = Decimal::from_str("1.1").unwrap();
    let rate_current_3 = Decimal::from_str("1.0").unwrap();
    let fee_3 = calculate_fee_to_mint(
        rate_old_3,
        rate_current_3,
        total_supply,
        fee_percentage,
        decimals,
    )
    .unwrap();
    assert_eq!(fee_3, Uint128::zero());

    // Case 4: Higher fee percentage (50%)
    // Target rate = 1.0 + (0.1 * 0.5) = 1.05
    // fee_dec = 1.0 * (1.1 / 1.05 - 1)
    // fee_dec = 1.0 * (0.047619...)
    // fee_dec = 0.047619...
    // Converting back to atomics (6 decimals) should give 47619
    let fee_percentage_4 = Decimal::from_str("0.5").unwrap(); // 50%
    let fee_4 = calculate_fee_to_mint(
        rate_old,
        rate_current,
        total_supply,
        fee_percentage_4,
        decimals,
    )
    .unwrap();
    assert_eq!(fee_4, Uint128::new(47619));
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

    assert_eq!(res.messages.len(), 0);

    // Assert: check attributes
    let expected_attributes = vec![
        Attribute::new("action", "instantiate"),
        Attribute::new("owner", msg.owner.clone()),
        Attribute::new("core_contract", msg.core_contract.clone()),
        Attribute::new("initial_exchange_rate", Decimal::one().to_string()),
    ];
    for attr in expected_attributes {
        assert!(
            res.attributes.contains(&attr),
            "Missing expected attribute: {} = {}",
            attr.key,
            attr.value
        );
    }
}

#[test]
fn test_collect_fee_success() {
    // Arrange
    let (mut deps, mut env, _) = setup_contract();

    let info = message_info(&deps.api.addr_make("owner_addr"), &[]);

    let cfg = CONFIG.load(&deps.storage).unwrap();

    // Simulate NextCollectionTime
    env.block.time = env.block.time.plus_seconds(100);

    deps.querier
        .set_exchange_rate(Decimal::from_str("1.05").unwrap());

    deps.querier
        .set_supply(&cfg.fee_denom, Uint128::new(100_000_000u128));

    // Act
    let resp = do_collect_fee(deps.as_mut(), env.clone(), info.clone()).unwrap();

    // Assert
    assert_eq!(
        resp,
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cfg.core_contract.to_string(),
                msg: to_json_binary(&CoreExecuteMsg::MintFee {
                    amount: Coin {
                        denom: cfg.fee_denom,
                        amount: Uint128::new(47_641u128),
                    },
                })
                .unwrap(),
                funds: vec![],
            }))
            .add_attribute("action", "collect_fee")
            .add_attribute("amount_to_mint", "47641")
            .add_attribute("new_exchange_rate", "1.05")
            .add_attribute("new_collection_timestamp", "1571797519.879305533")
    );
}

#[test]
fn test_collect_fee_collection_period_not_elapsed() {
    // Arrange
    let (mut deps, env, _) = setup_contract();

    let info = message_info(&deps.api.addr_make("owner_addr"), &[]);

    // Act
    let err = do_collect_fee(deps.as_mut(), env.clone(), info.clone()).unwrap_err();

    // Assert
    match err {
        ContractError::CollectionPeriodNotElapsed {} => (),
        e => panic!("Unexpected error: {e:?}"),
    }
}

#[test]
fn test_collect_fee_negative_or_zero_apy() {
    // Arrange
    let (mut deps, mut env, _) = setup_contract();

    let info = message_info(&deps.api.addr_make("owner_addr"), &[]);

    // Simulate NextCollectionTime
    env.block.time = env.block.time.plus_seconds(100);

    // Act
    let err = do_collect_fee(deps.as_mut(), env.clone(), info.clone()).unwrap_err();

    // Assert
    match err {
        ContractError::NegativeOrZeroApy {
            current_rate,
            last_rate,
        } => {
            assert_eq!(current_rate, Decimal::one());
            assert_eq!(last_rate, Decimal::one());
        }
        e => panic!("Unexpected error: {e:?}"),
    }
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
        core_contract: deps.api.addr_make("core_contract_addr").to_string(),
        fee_apy_reduction_percentage: Decimal::percent(1),
        collection_period_seconds: 10,
        fee_denom: "maxBTC".to_string(),
        maxbtc_decimals: 6,
    }
}

/// A convenience helper for calling the `execute_collect_fee` entry point.
fn do_collect_fee(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    execute(deps, env, info, ExecuteMsg::CollectFee {})
}
