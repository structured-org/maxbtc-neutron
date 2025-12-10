use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub factory_contract: Addr,
    pub core_contract: Addr,
    pub token_contract: Addr,
    pub deposit_denom: String,
}

#[cw_serde]
#[derive(Default)]
pub struct Pause {
    pub receive_nft_withdraw: bool,
}
pub const PAUSE: Item<Pause> = Item::new("pause");
pub const CONFIG: Item<Config> = Item::new("config");
pub const PAID_AMOUNT: Map<u64, Uint128> = Map::new("paid_amount");
