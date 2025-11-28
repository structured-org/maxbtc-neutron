use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Decimal, Int256};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateExchangeRate { rate: Decimal },
    UpdateAum { aum_in_wbtc: Int256, decimals: u32 },
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(GetTwaerResponse)]
    GetTwaer {},
    #[returns(GetAumResponse)]
    /// Returns the latest total AUM (in Bitcoin) reported by oracles.
    GetAum {},
}

#[cw_serde]
pub struct GetTwaerResponse {
    pub twaer: Decimal,
    pub published_at: u64,
}

/// Response type for AUM Oracle receiver GetAum queries. **Must** return the AUM in micro-Bitcoin
/// (uwBTC) with precision of `WBTC_DECIMALS`.
#[cw_serde]
pub struct GetAumResponse {
    /// The latest AUM in the AUM Oracle receiver reported by messengers scaled to `WBTC_DECIMALS`.
    pub aum_in_wbtc: Int256,
    /// Represents the number of decimals that the `aum_in_wbtc` is represented in. Must always
    /// equal to `WBTC_DECIMALS`. It is used to scale the `aum_in_wbtc` to its base BTC value.
    /// E.g. `base_aum_in_btc = aum_in_wbtc / 10^decimals`.
    pub decimals: u32,
}

#[cw_serde]
pub struct MigrateMsg {}
