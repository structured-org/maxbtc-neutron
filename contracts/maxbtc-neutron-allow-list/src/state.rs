use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

use crate::msg::ZkMeSettings;

pub const ALLOW_LIST_V1: Item<Vec<Addr>> = Item::new("allow_list");
pub const ALLOW_LIST: Map<&Addr, ()> = Map::new("allow_list_v2");
pub const ZK_ME_SETTINGS: Item<ZkMeSettings> = Item::new("zk_me_settings");
