use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::SignedDecimal256;

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(GetDataResponse)]
    GetData {},
}

#[cw_serde]
pub struct GetDataResponse {
    /// The latest published data (can be null if there was no consensus reached)
    pub last_published_data: Option<ConsensusOutcome<BinanceData>>,
}

#[cw_serde]
pub struct SpotBalance {
    pub asset: String,
    pub amount: SignedDecimal256,
}

#[cw_serde]
pub struct BinanceData {
    pub spot_balances: Vec<SpotBalance>,
}

#[cw_serde]
pub struct ConsensusOutcome<T> {
    /// The data submitted by the messengers
    pub data: T,
}
