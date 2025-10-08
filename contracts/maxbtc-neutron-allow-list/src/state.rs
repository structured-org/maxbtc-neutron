use cosmwasm_std::Addr;
use cw_storage_plus::Item;

use crate::msg::ZkMeSettings;

pub const ALLOW_LIST: Item<Vec<Addr>> = Item::new("allow_list");
pub const ZK_ME_SETTINGS: Item<ZkMeSettings> = Item::new("zk_me_settings");
