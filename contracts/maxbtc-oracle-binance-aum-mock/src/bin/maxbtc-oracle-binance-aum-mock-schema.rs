use cosmwasm_schema::write_api;

use maxbtc_oracle_binance_aum_mock::{msg::InstantiateMsg, state::QueryMsg};

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        query: QueryMsg,
    }
}
