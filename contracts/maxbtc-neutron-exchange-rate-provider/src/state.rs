use cosmwasm_std::{Decimal, Uint128};
use cw_storage_plus::Item;

pub const AUM: Item<Uint128> = Item::new("aum");
pub const EXCHANGE_RATE: Item<Decimal> = Item::new("exchange_rate");
