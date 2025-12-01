use cosmwasm_std::Decimal;
use cw_storage_plus::Item;

use crate::msg::GetAumResponse;

pub const EXCHANGE_RATE: Item<Decimal> = Item::new("exchange_rate");
pub const AUM: Item<GetAumResponse> = Item::new("aum");
