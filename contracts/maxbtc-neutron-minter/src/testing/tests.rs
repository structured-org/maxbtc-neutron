use crate::contract::instantiate;
use crate::msg::{InstantiateMsg, OperationOptions};
use crate::state::Config;
use crate::testing::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{message_info, mock_env};

#[test]
fn test_instantiate_success() {
    // Set up a mock environment with default values
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);

    // Create a sample InstantiateMsg
    let instantiate_msg = InstantiateMsg {
        apr: (),
        config: Config {
            name: "".to_string(),
            admin: (),
            base_token: "".to_string(),
            base_token_decimals: 0,
            vault_shares_token: "".to_string(),
            vault_shares_token_decimals: 0,
            checkpoint_duration: 0,
            position_duration: 0,
            time_stretch: (),
            curve_fee: (),
            flat_fee: (),
            governance_fee: (),
            governance_zombie_fee: (),
            initial_vault_share_price: (),
            minimum_share_reserves: (),
            minimum_transaction_amount: (),
            circuit_breaker_delta: (),
            yield_source_address: (),
        },
        contribution: Default::default(),
        options: OperationOptions {},
    };

    // Call the instantiate function
    instantiate(deps.as_mut(), env.clone(), info, instantiate_msg.clone())
        .expect("contract initialization should succeed");
}
