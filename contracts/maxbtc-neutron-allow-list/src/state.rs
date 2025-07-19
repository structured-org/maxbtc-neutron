use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

pub const ALLOW_LIST: Item<Vec<Addr>> = Item::new("allow_list");
pub const KYC_LIST: Map<&str, bool> = Map::new("kyc_list");
