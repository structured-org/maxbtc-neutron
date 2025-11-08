use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    coin, from_json, to_json_binary, Addr, BankQuery, Binary, Checksum, CodeInfoResponse, Coin,
    ContractResult, Decimal, Empty, GrpcQuery, Int256, OwnedDeps, Querier, QuerierResult,
    QueryRequest, SystemError, SystemResult, Uint128, WasmQuery,
};
use maxbtc_base::msg::core::{GetAumResponse, WaitosaurObserverQueryMsg};
use maxbtc_base::msg::{
    core::{AllowlistQueryMsg, ExchangeRateProviderQueryMsg, GetTwaerResponse},
    token::QueryMsg as TokenConfigQueryMsg,
    waitosaur_holder::QueryMsg as WaitosaurHolderQueryMsg,
};
use maxbtc_base::state::core::WaitosaurObserverState;
use maxbtc_base::state::{
    token::Config as TokenConfigResponse, waitosaur_holder::State as WaitsaurHolderState,
};
use neutron_std::types::osmosis::tokenfactory::v1beta1::QueryDenomAuthorityMetadataResponse;
use prost::Message;
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

    /// Optional map of `(address, denom) -> balance` for your own bank queries
    /// (used in `BankQuery::Balance { address, denom }`).
    balances: HashMap<(String, String), Uint128>,

    supplies: HashMap<String, Uint128>,

    allowed_recipient: bool,

    waitosaur_observer_state: WaitosaurObserverState,
    /// Amount of BTC to receive from CEFFU (for withdrawing batch)
    waitosaur_holder_state: WaitsaurHolderState,

    denom: String,

    denom_metadata: QueryDenomAuthorityMetadataResponse,

    aum_in_wbtc: Int256,

    exchange_rate: (Decimal, u64),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct BalanceResponse {
    /// Always returns a Coin with the requested denom.
    /// This may be of 0 amount if no such funds.
    pub amount: Coin,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
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
            balances: HashMap::new(),
            supplies: HashMap::new(),
            allowed_recipient: true,
            waitosaur_holder_state: WaitsaurHolderState::Unlocked {},
            denom: "maxBTC".to_string(),
            aum_in_wbtc: Int256::zero(),
            exchange_rate: (Decimal::one(), 0),
            waitosaur_observer_state: WaitosaurObserverState::Unlocked {},
            denom_metadata: QueryDenomAuthorityMetadataResponse::default(),
        }
    }

    /// Allows you to store any arbitrary `(address, denom) -> amount` for `BankQuery::Balance`.
    pub fn set_balance(&mut self, address: &str, denom: &str, amount: Uint128) {
        self.balances.insert((address.into(), denom.into()), amount);
    }

    pub fn set_supply(&mut self, denom: &str, amount: Uint128) {
        self.supplies.insert(denom.into(), amount);
    }

    pub fn set_allowed_recipient(&mut self, allowed: bool) {
        self.allowed_recipient = allowed;
    }

    pub fn set_waitosaur_observer_state(&mut self, state: WaitosaurObserverState) {
        self.waitosaur_observer_state = state;
    }

    pub fn set_waitsaur_holder_state(&mut self, state: WaitsaurHolderState) {
        self.waitosaur_holder_state = state;
    }

    pub fn set_exchange_rate(&mut self, exchange_rate: (Decimal, u64)) {
        self.exchange_rate = exchange_rate;
    }

    pub fn set_aum_in_wbtc(&mut self, aum_in_wbtc: Int256) {
        self.aum_in_wbtc = aum_in_wbtc;
    }

    pub fn set_denom_metadata(&mut self, denom_metadata: QueryDenomAuthorityMetadataResponse) {
        self.denom_metadata = denom_metadata;
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
                    Checksum::generate(&[1, 2, 3, 4, 5]),
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
        println!("Handling wasm query for contract: {contract_addr}");

        // Exchange rate provider contract
        if contract_addr == "cosmwasm1qugtqqz3w5yqdt7z56nx5aj0umrtz2q4r8escthjpgkvmhdjkypq9umkdq" {
            let parsed: Result<ExchangeRateProviderQueryMsg, _> = from_json(msg);
            if let Ok(q) = parsed {
                return match q {
                    ExchangeRateProviderQueryMsg::GetTwaer {} => {
                        let val = self.exchange_rate;
                        SystemResult::Ok(ContractResult::Ok(
                            to_json_binary(&GetTwaerResponse {
                                twaer: val.0,
                                published_at: val.1,
                            })
                            .unwrap(),
                        ))
                    }
                    ExchangeRateProviderQueryMsg::GetAum {} => {
                        let val = self.aum_in_wbtc;
                        SystemResult::Ok(ContractResult::Ok(
                            to_json_binary(&GetAumResponse {
                                aum_in_wbtc: val,
                                decimals: 8,
                            })
                            .unwrap(),
                        ))
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

        // 2. Check if it's the allowlist contract
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

        // Waitosaur contract
        if contract_addr == "cosmwasm1603h02gmafrs2ar32mcx83885aqt8yms86smppjstl223swgyjps0f242x" {
            let parsed: Result<WaitosaurObserverQueryMsg, _> = from_json(msg);
            if let Ok(q) = parsed {
                return match q {
                    WaitosaurObserverQueryMsg::GetState {} => {
                        let val = self.waitosaur_observer_state.clone();
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

        // Waitosaur holder contract
        if contract_addr == "cosmwasm1nylrq8x440yzqme262zy5875tt7vyn5yghjg5u807gms0359zl9svnrlrp" {
            let parsed_query_msg: Result<WaitosaurHolderQueryMsg, _> = from_json(msg);
            if let Ok(q) = parsed_query_msg {
                return match q {
                    WaitosaurHolderQueryMsg::GetState { .. } => SystemResult::Ok(
                        ContractResult::Ok(to_json_binary(&self.waitosaur_holder_state).unwrap()),
                    ),
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

        // Query token contract
        if contract_addr == "cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw" {
            let parsed: Result<TokenConfigQueryMsg, _> = from_json(msg);
            if let Ok(q) = parsed {
                return match q {
                    TokenConfigQueryMsg::GetDenom { subdenom } => {
                        SystemResult::Ok(ContractResult::Ok(
                            to_json_binary(&format!("factory/cosmwasm1sc3nrdnvngw79j0rkwm5zyaa46r6546h2ypz8skfnvnhpanmg2fsryrwsw/{}", subdenom.unwrap_or("maxbtc".to_string()))).unwrap(),
                        ))
                    }
                    TokenConfigQueryMsg::Config {} => SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(&TokenConfigResponse {
                            factory_contract: Addr::unchecked("factory"),
                            denom: self.denom.clone(),
                        })
                        .unwrap(),
                    )),
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

        // 3. If it's not one of our recognized addresses, fallback to base
        self.base
            .handle_query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: contract_addr.into(),
                msg: msg.clone(),
            }))
    }

    fn handle_grpc_query(&self, grpc_query: GrpcQuery) -> QuerierResult {
        println!("Handling grpc query for path: {}", grpc_query.path);

        match grpc_query.path.as_str() {
            "/osmosis.tokenfactory.v1beta1.Query/DenomAuthorityMetadata" => {
                let bytes = self.denom_metadata.encode_to_vec();

                let query_result: ContractResult<Binary> = ContractResult::Ok(Binary::from(bytes));

                SystemResult::Ok(query_result)
            }
            _ => self.base.handle_query(&QueryRequest::Grpc(grpc_query)),
        }
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
                    error: format!("Parsing query request: {e}"),
                    request: bin_request.into(),
                })
            }
        };

        // Now match on it:
        match request {
            QueryRequest::Bank(bank_query) => self.handle_bank_query(bank_query),
            QueryRequest::Wasm(wasm_query) => self.handle_wasm_query(wasm_query),
            QueryRequest::Grpc(grpc_query) => self.handle_grpc_query(grpc_query),
            // Fallback for queries we don’t explicitly handle
            _ => self.base.handle_query(&request),
        }
    }
}
