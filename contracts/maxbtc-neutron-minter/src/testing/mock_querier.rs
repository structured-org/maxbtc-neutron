use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    from_json, Binary, ContractResult, Empty, GrpcQuery, OwnedDeps, Querier, QuerierResult,
    QueryRequest, SystemError, SystemResult,
};
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
pub struct WasmMockQuerier {
    base: MockQuerier,
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier) -> Self {
        WasmMockQuerier { base }
    }

    fn handle_grpc_query(&self, path: &str, _data: &[u8]) -> SystemResult<ContractResult<Binary>> {
        match path {
            "/slinky.module.v1.Query/GetStuff" => {
                unimplemented!()
            }
            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                kind: format!("Unhandled path in mock_querier: {}", path),
            }),
        }
    }

    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Grpc(GrpcQuery { data, path }) => self.handle_grpc_query(path, data),
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
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                });
            }
        };
        self.handle_query(&request)
    }
}
