use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    coin, from_json, to_json_binary, BankQuery, Binary, Coin, ContractResult, Empty, OwnedDeps,
    Querier, QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmQuery,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub struct BalanceResponse {
    /// Always returns a Coin with the requested denom.
    /// This may be of 0 amount if no such funds.
    pub amount: Coin,
}

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

/// A custom mock querier that can handle WASM queries.
pub struct WasmMockQuerier {
    base: MockQuerier<Empty>,
    // Map from contract_addr => response binary for WASM queries
    pub wasm_responses: HashMap<String, Binary>,
    balances: HashMap<(String, String), Uint128>,
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<Empty>) -> Self {
        WasmMockQuerier {
            base,
            wasm_responses: HashMap::new(),
            balances: HashMap::new(),
        }
    }

    pub fn set_balance(&mut self, address: &str, denom: &str, amount: Uint128) {
        self.balances.insert((address.into(), denom.into()), amount);
    }

    fn handle_wasm_query(&self, wasm_query: &WasmQuery) -> SystemResult<ContractResult<Binary>> {
        match wasm_query {
            WasmQuery::Smart { contract_addr, .. } => {
                if let Some(response) = self.wasm_responses.get(contract_addr) {
                    SystemResult::Ok(ContractResult::Ok(response.clone()))
                } else {
                    SystemResult::Err(SystemError::UnsupportedRequest {
                        kind: format!("No mock response for contract: {contract_addr}"),
                    })
                }
            }
            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                kind: "Unsupported WASM query".to_string(),
            }),
        }
    }

    fn handle_bank_query(&self, query: &BankQuery) -> QuerierResult {
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

            // For other queries, fallback to base
            _ => self.base.handle_query(&QueryRequest::Bank(query.clone())),
        }
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
