use crate::contract::instantiate;
use crate::msg::InstantiateMsg;
use crate::state::{
    ContractState, ACTIVE_BATCH, ACTIVE_BATCH_START_TIME, BATCH_ID_COUNTER, CONFIG, FSM,
    LAST_DEPOSIT_FLUSH_TIME,
};
use crate::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockStorage};
use cosmwasm_std::{Attribute, Decimal, OwnedDeps, Uint128};

fn default_instantiate_msg(
    deps: &OwnedDeps<MockStorage, MockApi, WasmMockQuerier>,
) -> InstantiateMsg {
    InstantiateMsg {
        owner: deps.api.addr_make("owner_addr").to_string(),
        aum_contract: deps.api.addr_make("aum_addr").to_string(),
        liquidation_contract: deps.api.addr_make("liq_buffer_addr").to_string(),
        deposit_pump_contract: deps.api.addr_make("pump_addr").to_string(),
        collector_contract: deps.api.addr_make("collector_addr").to_string(),
        treasury_address: deps.api.addr_make("treasury_addr").to_string(),
        deposit_denom: "wBTC".to_string(),
        deposit_decimals: 6u32,
        maxbtc_denom: "maxbtc".to_string(),
        deposit_flush_period: 3600,
        batch_active_duration: 86400,
        batch_withdrawing_duration: 86400,
        accepted_withdrawable_percentage: Decimal::percent(5),
        liquidation_buffer_share: Decimal::percent(10),
        deposit_fee: Decimal::percent(1),
        cached_aum_tolerance: Decimal::percent(2),
        cached_er_ttl: 100u64,
        deposits_cap: None,
        deposits_allowlist: None,
    }
}

#[test]
fn test_instantiate_success() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("some_sender"), &[]);

    // Arrange: build a valid instantiate msg
    let msg = default_instantiate_msg(&deps);

    // Act: call instantiate
    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    // Assert: check response
    // We expect 1 message: create_tokenfactory_create_denom_msg
    assert_eq!(res.messages.len(), 1);
    // Assert: check attributes
    let expected_attributes = vec![
        Attribute::new("action", "instantiate"),
        Attribute::new("owner", msg.owner.clone()),
        Attribute::new("aum_contract", msg.aum_contract.clone()),
        Attribute::new(
            "liquidation_buffer_contract",
            msg.liquidation_contract.clone(),
        ),
        Attribute::new("collector_contract", msg.collector_contract.clone()),
        Attribute::new("treasury_address", msg.treasury_address.clone()),
        Attribute::new("deposit_denom", msg.deposit_denom.clone()),
        Attribute::new("maxbtc_denom", msg.maxbtc_denom.clone()),
        Attribute::new("deposit_flush_period", msg.deposit_flush_period.to_string()),
        Attribute::new(
            "batch_active_duration",
            msg.batch_active_duration.to_string(),
        ),
        Attribute::new(
            "batch_withdrawing_duration",
            msg.batch_withdrawing_duration.to_string(),
        ),
        Attribute::new(
            "accepted_withdrawable_percentage",
            msg.accepted_withdrawable_percentage.to_string(),
        ),
        Attribute::new(
            "liquidation_buffer_share",
            msg.liquidation_buffer_share.to_string(),
        ),
        Attribute::new("deposit_fee", msg.deposit_fee.to_string()),
    ];
    for attr in expected_attributes {
        assert!(
            res.attributes.contains(&attr),
            "Missing expected attribute: {} = {}",
            attr.key,
            attr.value
        );
    }

    // Assert: check contract storage
    let cfg = CONFIG.load(&deps.storage).unwrap();
    assert_eq!(cfg.owner, deps.api.addr_make("owner_addr"));
    assert_eq!(cfg.aum_contract, &deps.api.addr_make("aum_addr"));
    assert_eq!(cfg.paused, false);
    // etc. check more fields
    assert_eq!(cfg.deposit_decimals, 6u32);
    assert_eq!(cfg.deposit_denom, "wBTC");
    assert_eq!(cfg.liquidation_buffer_share, Decimal::percent(10));

    let counter = BATCH_ID_COUNTER.load(&deps.storage).unwrap();
    assert_eq!(counter, 1u64);

    let active_batch = ACTIVE_BATCH.load(&deps.storage).unwrap();
    assert!(active_batch.is_some());
    let active_batch_data = active_batch.unwrap();
    assert_eq!(active_batch_data.batch_id, 1u64);
    assert_eq!(active_batch_data.btc_requested, Uint128::zero());
    assert_eq!(active_batch_data.maxbtc_burned, Uint128::zero());
    assert_eq!(active_batch_data.collected_amount, Uint128::zero());
    assert_eq!(active_batch_data.paid_amount, Uint128::zero());

    let fsm_state = FSM.get_current_state(&deps.storage).unwrap();
    assert_eq!(fsm_state, ContractState::Idle);

    // Finally, check time-based items
    let last_deposit_flush_time = LAST_DEPOSIT_FLUSH_TIME.load(&deps.storage).unwrap();
    assert_eq!(last_deposit_flush_time, env.block.time.seconds());
    let active_batch_start_time = ACTIVE_BATCH_START_TIME.load(&deps.storage).unwrap();
    assert_eq!(active_batch_start_time, env.block.time.seconds());
}
