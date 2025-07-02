use cosmwasm_schema::cw_serde;

// Define the query message for the core contract
#[cw_serde]
pub enum CoreQueryMsg {
    ExchangeRate {},
}
