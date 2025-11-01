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
    pub unimmr: SignedDecimal256,
    pub positions: Vec<Position>,
    pub um_balance_usdt: SignedDecimal256,
    pub pm_account_actual_equity: SignedDecimal256,
    pub withdrawable_usdt: SignedDecimal256,
    pub spot_balances: Vec<SpotBalance>,
}

#[cw_serde]
pub struct Position {
    pub symbol: String,
    pub amount: SignedDecimal256,
    pub pnl: SignedDecimal256,
}

#[cw_serde]
pub struct ConsensusOutcome<T> {
    pub round: u64,
    pub timestamp: u64,
    pub data: T,
}
