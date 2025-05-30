use crate::msg::{LiquidationBufferContractQueryMsg, OracleQueryMsg};
use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    coin, from_json, to_json_binary, BankQuery, Binary, Coin, ContractResult, Empty, OwnedDeps,
    Querier, QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmQuery,
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

    /// Mocked total maxBTC supply for `QueryRequest::Bank(BankQuery::Supply { denom })`.
    maxbtc_supply: Uint128,

    /// Mocked liquidation buffer contract's `GetMaxBTCBalance`.
    liqbuffer_maxbtc_balance: Uint128,

    /// Mocked liquidation buffer contract's `GetBTCBalance`.
    liqbuffer_btc_balance: Uint128,

    /// Optional map of `(address, denom) -> balance` for your own bank queries
    /// (used in `BankQuery::Balance { address, denom }`).
    balances: HashMap<(String, String), Uint128>,
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
            maxbtc_supply: Uint128::zero(),
            liqbuffer_maxbtc_balance: Uint128::zero(),
            liqbuffer_btc_balance: Uint128::zero(),
            balances: HashMap::new(),
        }
    }

    // ---------- Update methods for mocking specific values ----------
    pub fn update_oracle_aum(&mut self, val: Uint128) {
        self.oracle_aum = val;
    }

    pub fn update_maxbtc_supply(&mut self, val: Uint128) {
        self.maxbtc_supply = val;
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

    /// Convenience: mock both BTC *and* maxBTC balances in the liquidation buffer
    /// in a single call (useful for the new tests).
    pub fn update_liqbuffer_balances(&mut self, btc: Uint128, maxbtc: Uint128) {
        self.liqbuffer_btc_balance = btc;
        self.liqbuffer_maxbtc_balance = maxbtc;
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
                // If this is the "maxbtc" denom, return our mocked maxbtc_supply
                if denom == "maxbtc" {
                    let coin = coin(self.maxbtc_supply.u128(), denom.clone());
                    let resp = SupplyResponse { amount: coin };
                    SystemResult::Ok(ContractResult::Ok(to_json_binary(&resp).unwrap()))
                } else {
                    // Fallback: let the base mock handle it (could be zero or whatever default)
                    self.base
                        .handle_query(&QueryRequest::Bank(BankQuery::Supply { denom }))
                }
            }
            // For other queries, fallback to base
            _ => self.base.handle_query(&QueryRequest::Bank(query)),
        }
    }

    fn handle_wasm_query(&self, wasm_query: WasmQuery) -> QuerierResult {
        match wasm_query {
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
                match q {
                    LiquidationBufferContractQueryMsg::GetMaxBTCBalance {} => {
                        let val = self.liqbuffer_maxbtc_balance;
                        return SystemResult::Ok(ContractResult::Ok(to_json_binary(&val).unwrap()));
                    }
                    LiquidationBufferContractQueryMsg::GetBTCBalance {} => {
                        let val = self.liqbuffer_btc_balance;
                        return SystemResult::Ok(ContractResult::Ok(to_json_binary(&val).unwrap()));
                    }
                }
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
