use crate::msg::{AllowlistQueryMsg, LiquidationBufferContractQueryMsg, OracleQueryMsg};
use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    coin, from_json, to_json_binary, Addr, BankQuery, Binary, Checksum, CodeInfoResponse, Coin,
    ContractResult, Empty, OwnedDeps, Querier, QuerierResult, QueryRequest, SystemError,
    SystemResult, Uint128, WasmQuery,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;

pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_storage = MockStorage::default();
    let custom_querier = WasmMockQuerier::new(MockQuerier::new(&[]));

    OwnedDeps {
        storage: custom_storage,
        api: MockApi::default(),
        querier: custom_querier,
        custom_query_type: PhantomData,
    }
}

/// A custom mock querier.
/// A custom mock querier that extends `MockQuerier`.
/// It can intercept queries to return controlled values (AUM, liquidation balances, etc.).
pub struct WasmMockQuerier {
    /// The standard cosmwasm mock querier (handles most queries by default).
    base: MockQuerier<Empty>,

    /// Mocked AUM value for `OracleQueryMsg::GetAUM {}` queries.
    oracle_aum: Uint128,

    /// Mocked liquidation buffer contract's `GetMaxBTCBalance`.
    liqbuffer_maxbtc_balance: Uint128,

    /// Mocked liquidation buffer contract's `GetBTCBalance`.
    liqbuffer_btc_balance: Uint128,

    /// Optional map of `(address, denom) -> balance` for your own bank queries
    /// (used in `BankQuery::Balance { address, denom }`).
    balances: HashMap<(String, String), Uint128>,

    supplies: HashMap<String, Uint128>,

    allowed_recipient: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub struct BalanceResponse {
    /// Always returns a Coin with the requested denom.
    /// This may be of 0 amount if no such funds.
    pub amount: Coin,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub struct SupplyResponse {
    /// Always returns a Coin with the requested denom.
    /// This will be of zero amount if the denom does not exist.
    pub amount: Coin,
}

/// Implementation of our custom querier.
impl WasmMockQuerier {
    /// Construct a new `WasmMockQuerier`, wrapping the base `MockQuerier`.
    pub fn new(base: MockQuerier<Empty>) -> Self {
        WasmMockQuerier {
            base,
            oracle_aum: Uint128::zero(),
            liqbuffer_maxbtc_balance: Uint128::zero(),
            liqbuffer_btc_balance: Uint128::zero(),
            balances: HashMap::new(),
            supplies: Default::default(),
            allowed_recipient: true,
        }
    }

    // ---------- Update methods for mocking specific values ----------
    pub fn update_oracle_aum(&mut self, val: Uint128) {
        self.oracle_aum = val;
    }

    pub fn update_liqbuffer_maxbtc_balance(&mut self, val: Uint128) {
        self.liqbuffer_maxbtc_balance = val;
    }

    pub fn update_liqbuffer_btc_balance(&mut self, val: Uint128) {
        self.liqbuffer_btc_balance = val;
    }

    /// Allows you to store any arbitrary `(address, denom) -> amount` for `BankQuery::Balance`.
    pub fn set_balance(&mut self, address: &str, denom: &str, amount: Uint128) {
        self.balances.insert((address.into(), denom.into()), amount);
    }

    pub fn set_token_supply(&mut self, denom: &str, amount: Uint128) {
        self.supplies.insert(denom.into(), amount);
    }

    pub fn set_allowed_recipient(&mut self, allowed: bool) {
        self.allowed_recipient = allowed;
    }

    // ---------- Implementation of the Querier trait ----------
    fn handle_bank_query(&self, query: BankQuery) -> QuerierResult {
        match query {
            BankQuery::Balance { address, denom } => {
                // Return the stored balance if set, else zero
                let amount = self
                    .balances
                    .get(&(address.clone(), denom.clone()))
                    .copied()
                    .unwrap_or_else(Uint128::zero);
                // Build response
                let resp = BalanceResponse {
                    amount: coin(amount.u128(), denom.clone()),
                };
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&resp).unwrap()))
            }
            BankQuery::Supply { denom } => {
                if let Some(val) = self.supplies.get(&denom) {
                    let resp = SupplyResponse {
                        amount: coin(val.u128(), denom.clone()),
                    };
                    return SystemResult::Ok(ContractResult::Ok(to_json_binary(&resp).unwrap()));
                }

                self.base
                    .handle_query(&QueryRequest::Bank(BankQuery::Supply { denom }))
            }
            // For other queries, fallback to base
            _ => self.base.handle_query(&QueryRequest::Bank(query)),
        }
    }

    fn handle_wasm_query(&self, wasm_query: WasmQuery) -> QuerierResult {
        match wasm_query {
            WasmQuery::CodeInfo { .. } => {
                let query_result: ContractResult<Binary> = to_json_binary(&CodeInfoResponse::new(
                    0,
                    Addr::unchecked("creator"),
                    Checksum::generate(&vec![1, 2, 3, 4, 5]),
                ))
                .into();
                SystemResult::Ok(query_result)
            }
            WasmQuery::Smart { contract_addr, msg } => {
                // Attempt to decode into one of our known query message types
                self.handle_wasm_smart_query(&contract_addr, &msg)
            }
            // For other variants (e.g. WasmQuery::Raw), fallback to base
            _ => self.base.handle_query(&QueryRequest::Wasm(wasm_query)),
        }
    }

    /// Dispatches our recognized wasm queries to the correct mock data.
    fn handle_wasm_smart_query(&self, contract_addr: &str, msg: &Binary) -> QuerierResult {
        println!("Handling wasm query for contract: {}", contract_addr);
        // 1. Check if it's the AUM contract
        if contract_addr == "cosmwasm1cytjkuu7je7h7tx7wa9u9fk9tca38zhv67kk5xgeez6jkjlrec9qk7w0nz" {
            // Try parse as OracleQueryMsg
            let parsed: Result<OracleQueryMsg, _> = from_json(msg);
            if let Ok(q) = parsed {
                return match q {
                    OracleQueryMsg::GetAUM {} => {
                        // Return the mocked `aum` value
                        let val = self.oracle_aum;
                        SystemResult::Ok(ContractResult::Ok(to_json_binary(&val).unwrap()))
                    }
                };
            }
            // If parse failed or unsupported query => fallback
            return self
                .base
                .handle_query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: contract_addr.into(),
                    msg: msg.clone(),
                }));
        }

        // 2. Check if it's the Liquidation Buffer contract
        if contract_addr == "cosmwasm1cse2m3gz5qxynp4t5hh5sg0zyn9gskys3u9ac360qmftvhw6pqlqutnz9j" {
            let parsed: Result<LiquidationBufferContractQueryMsg, _> = from_json(msg);
            if let Ok(q) = parsed {
                return match q {
                    LiquidationBufferContractQueryMsg::GetMaxBTCBalance {} => {
                        let val = self.liqbuffer_maxbtc_balance;
                        SystemResult::Ok(ContractResult::Ok(to_json_binary(&val).unwrap()))
                    }
                    LiquidationBufferContractQueryMsg::GetBTCBalance {} => {
                        let val = self.liqbuffer_btc_balance;
                        SystemResult::Ok(ContractResult::Ok(to_json_binary(&val).unwrap()))
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
        // 3. Check if it's the allowlist contract
        if contract_addr == "cosmwasm1gpvxj2y5ungxeykk57hqthzuahfchunqjexzsflt962fjr8yc3cqyh0fg8" {
            let parsed: Result<AllowlistQueryMsg, _> = from_json(msg);
            if let Ok(q) = parsed {
                return match q {
                    AllowlistQueryMsg::IsAddressAllowed { .. } => {
                        let val = self.allowed_recipient;
                        SystemResult::Ok(ContractResult::Ok(to_json_binary(&val).unwrap()))
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

        // 3. If it's not one of our recognized addresses, fallback to base
        self.base
            .handle_query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: contract_addr.into(),
                msg: msg.clone(),
            }))
    }
}

/// Finally, implement the high-level `Querier` trait that calls
/// our sub‐handlers for Bank or Wasm queries.
impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // First, parse the incoming request into a `QueryRequest`
        let request: QueryRequest<Empty> = match from_json(bin_request) {
            Ok(req) => req,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };

        // Now match on it:
        match request {
            QueryRequest::Bank(bank_query) => self.handle_bank_query(bank_query),
            QueryRequest::Wasm(wasm_query) => self.handle_wasm_query(wasm_query),
            // Fallback for queries we don’t explicitly handle
            _ => self.base.handle_query(&request),
        }
    }
}
