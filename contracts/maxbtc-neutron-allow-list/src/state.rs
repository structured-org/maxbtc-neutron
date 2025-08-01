use cosmwasm_std::Addr;
use cw_storage_plus::Item;

pub const ALLOW_LIST: Item<Vec<Addr>> = Item::new("allow_list");
