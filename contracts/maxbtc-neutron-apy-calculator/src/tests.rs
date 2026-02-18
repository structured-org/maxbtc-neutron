use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use crate::contract::{execute_update_exchange_rates, instantiate, predict_apy, query};
use crate::error::ContractError;
use crate::msgs::{InstantiateMsg, QueryMsg};
use crate::state::{
    Apy, Config, CoreQueryMsg, ExchangeRate, CONFIG, EXCHANGE_RATE, LAST_ER_UPDATE,
    NEXT_HOUR_COUNTER,
};
use cosmwasm_std::testing::{mock_dependencies, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    attr, from_json,
    testing::{message_info, mock_env},
    Addr, Event, Response,
};
use cosmwasm_std::{
    to_json_binary, ContractResult, Decimal, OwnedDeps, StdError, SystemError, SystemResult,
    Timestamp, WasmQuery,
};
use cw_ownable::Ownership;

#[test]
fn test_predict_apy() {
    use cosmwasm_std::Timestamp;
    use std::str::FromStr;

    let test_data = vec![
        (
            ExchangeRate {
                height: 1,
                exchange_rate: Decimal::one(),
                timestamp: Timestamp::from_seconds(1),
            },
            ExchangeRate {
                height: 10,
                exchange_rate: Decimal::from_str("2.0").unwrap(),
                timestamp: Timestamp::from_seconds(86400),
            },
            Decimal::from_str("365.004224585932707554").unwrap(),
        ),
        (
            ExchangeRate {
                height: 1,
                exchange_rate: Decimal::one(),
                timestamp: Timestamp::from_seconds(1),
            },
            ExchangeRate {
                height: 10,
                exchange_rate: Decimal::from_str("1.01").unwrap(),
                timestamp: Timestamp::from_seconds(2592000),
            },
            Decimal::from_str("0.121666713605985187").unwrap(),
        ),
        (
            ExchangeRate {
                height: 1,
                exchange_rate: Decimal::one(),
                timestamp: Timestamp::from_seconds(1),
            },
            ExchangeRate {
                height: 10,
                exchange_rate: Decimal::from_str("1.00000023").unwrap(),
                timestamp: Timestamp::from_seconds(15552000),
            },
            Decimal::from_str("0.000000466388918877").unwrap(),
        ),
        (
            ExchangeRate {
                height: 31687541,
                exchange_rate: Decimal::from_str("1.005776802074055183").unwrap(),
                timestamp: Timestamp::from_nanos(1750880678975816708),
            },
            ExchangeRate {
                height: 31718524,
                exchange_rate: Decimal::from_str("1.005776802074177447").unwrap(),
                timestamp: Timestamp::from_nanos(1750923797332357316),
            },
            Decimal::from_str("0.000000000088906229").unwrap(),
        ),
    ];
    for (start, end, expected) in test_data {
        let apy = predict_apy(&start, &end).unwrap();
        assert_eq!(apy, expected);
    }
}

#[test]
fn test_instantiate_general() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let sender = deps.api.addr_make("sender");
    let core_contract = deps.api.addr_make("core_contract1").to_string();
    let deps_mut = deps.as_mut();
    let response = instantiate(
        deps_mut,
        mock_env(),
        message_info(&sender, &[]),
        InstantiateMsg {
            owner: owner.to_string(),
            period_timeout: 1,
            core_contract,
            periods_to_keep: 1,
        },
    )
    .unwrap();

    assert_eq!(
        response,
        Response::new().add_event(
            Event::new("maxbtc-neutron-apy-calculator-instantiate").add_attributes(vec![
                attr("msg", format!("InstantiateMsg {{ owner: \"{}\", core_contract: \"cosmwasm1c6u47qaftrcxnsuszrnpj45z83v2sxxldl68h7f488g4s4mz50tsnmgg5l\", period_timeout: 1, periods_to_keep: 1 }}", owner)),
            ])
        )
    );

    let res: Config = from_json(
        query(
            deps.as_ref().into_empty(),
            mock_env(),
            QueryMsg::GetConfig {},
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        res,
        Config {
            core_contract: deps.api.addr_make("core_contract1"),
            periods_to_keep: 1,
            period_timeout: 1
        }
    );
}

#[test]
fn test_query_owner() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");

    let deps_mut = deps.as_mut();
    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some(owner.as_ref())).unwrap();

    let owner_data: Ownership<Addr> = from_json(
        query(
            deps.as_ref().into_empty(),
            mock_env(),
            QueryMsg::Ownership {},
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(owner_data.owner, Some(owner));
}

#[test]
fn test_query_apy() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let core_contract = deps.api.addr_make("core_contract1");

    let deps_mut = deps.as_mut();
    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some(owner.as_ref())).unwrap();

    CONFIG
        .save(
            deps_mut.storage,
            &Config {
                period_timeout: 1,
                core_contract,
                periods_to_keep: 2,
            },
        )
        .unwrap();

    EXCHANGE_RATE
        .save(
            deps_mut.storage,
            0,
            &ExchangeRate {
                height: 10,
                exchange_rate: Decimal::one(),
                timestamp: Timestamp::from_seconds(1),
            },
        )
        .unwrap();

    EXCHANGE_RATE
        .save(
            deps_mut.storage,
            1,
            &ExchangeRate {
                height: 100,
                exchange_rate: Decimal::from_str("2.0").unwrap(),
                timestamp: Timestamp::from_seconds(86400),
            },
        )
        .unwrap();

    NEXT_HOUR_COUNTER.save(deps_mut.storage, &2).unwrap();

    let response: Apy = from_json(
        query(
            deps.as_ref().into_empty(),
            mock_env(),
            QueryMsg::GetApy { time_span_hours: 1 },
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        response,
        Apy {
            start_exchange_rate: ExchangeRate {
                height: 10,
                exchange_rate: Decimal::one(),
                timestamp: Timestamp::from_seconds(1),
            },
            end_exchange_rate: ExchangeRate {
                height: 100,
                exchange_rate: Decimal::from_str("2.0").unwrap(),
                timestamp: Timestamp::from_seconds(86400),
            },
            apy: Decimal::from_str("365.004224585932707554").unwrap(),
        }
    );
}

#[test]
fn test_query_apy_time_span_hours_out_of_range() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let core_contract = deps.api.addr_make("core_contract1");

    let deps_mut = deps.as_mut();
    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some(owner.as_ref())).unwrap();

    CONFIG
        .save(
            deps_mut.storage,
            &Config {
                period_timeout: 1,
                core_contract,
                periods_to_keep: 2,
            },
        )
        .unwrap();

    EXCHANGE_RATE
        .save(
            deps_mut.storage,
            0,
            &ExchangeRate {
                height: 10,
                exchange_rate: Decimal::one(),
                timestamp: Timestamp::from_seconds(1),
            },
        )
        .unwrap();

    EXCHANGE_RATE
        .save(
            deps_mut.storage,
            1,
            &ExchangeRate {
                height: 100,
                exchange_rate: Decimal::from_str("2.0").unwrap(),
                timestamp: Timestamp::from_seconds(86400),
            },
        )
        .unwrap();

    EXCHANGE_RATE
        .save(
            deps_mut.storage,
            1,
            &ExchangeRate {
                height: 100,
                exchange_rate: Decimal::from_str("2.0").unwrap(),
                timestamp: Timestamp::from_seconds(86400),
            },
        )
        .unwrap();

    NEXT_HOUR_COUNTER.save(deps_mut.storage, &3).unwrap();

    let result_err = query(
        deps.as_ref().into_empty(),
        mock_env(),
        QueryMsg::GetApy { time_span_hours: 3 },
    )
    .unwrap_err();

    assert_eq!(result_err, ContractError::TimespanOutOfRange {});
}

#[test]
fn test_update_exchange_rate_too_early() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let core_contract = deps.api.addr_make("core_contract1");

    let deps_mut = deps.as_mut();

    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some(owner.as_ref())).unwrap();

    CONFIG
        .save(
            deps_mut.storage,
            &Config {
                period_timeout: 1000,
                core_contract,
                periods_to_keep: 2,
            },
        )
        .unwrap();

    let mocked_env = mock_env();
    LAST_ER_UPDATE
        .save(deps_mut.storage, &mocked_env.block.time)
        .unwrap();

    let result_err =
        execute_update_exchange_rates(deps_mut, mocked_env, message_info(&owner, &[])).unwrap_err();

    assert_eq!(result_err, ContractError::TooEarly {});
}

#[test]
fn test_update_exchange_rate_too_early_after_second_call() {
    let mut querier: MockQuerier<cosmwasm_std::Empty> = MockQuerier::new(&[]);
    querier.update_wasm(move |query| match query {
        WasmQuery::Smart {
            contract_addr: _,
            msg: _,
        } => SystemResult::Ok(ContractResult::Ok(to_json_binary(&Decimal::one()).unwrap())),
        _ => panic!("Unexpected query"),
    });

    let mut deps: OwnedDeps<_, _, MockQuerier, cosmwasm_std::Empty> = OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier,
        custom_query_type: std::marker::PhantomData,
    };
    let owner = deps.api.addr_make("owner");
    let core_contract = deps.api.addr_make("core_contract1");

    let deps_mut = deps.as_mut();

    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some(owner.as_ref())).unwrap();

    CONFIG
        .save(
            deps_mut.storage,
            &Config {
                period_timeout: 10,
                core_contract,
                periods_to_keep: 2,
            },
        )
        .unwrap();

    NEXT_HOUR_COUNTER.save(deps_mut.storage, &0).unwrap();

    let mocked_env = mock_env();
    LAST_ER_UPDATE
        .save(deps_mut.storage, &Timestamp::from_seconds(0))
        .unwrap();

    execute_update_exchange_rates(deps_mut, mocked_env.clone(), message_info(&owner, &[])).unwrap();

    let result_err = execute_update_exchange_rates(
        deps.as_mut(),
        mocked_env,
        message_info(&Addr::unchecked("wrong_sender"), &[]),
    )
    .unwrap_err();

    assert_eq!(result_err, ContractError::TooEarly {});
}

#[test]
fn test_update_exchange_rate_two_hours_and_query() {
    let contract_addr = "cosmwasm1c6u47qaftrcxnsuszrnpj45z83v2sxxldl68h7f488g4s4mz50tsnmgg5l";

    let exchange_rates = vec![
        Decimal::from_str("1.01").unwrap(),
        Decimal::from_str("1.02").unwrap(),
        Decimal::from_str("1.03").unwrap(),
    ];
    let cloned_exchange_rates = exchange_rates.clone();

    let call_count = Rc::new(RefCell::new(0usize));
    let count_ref = call_count.clone();

    let mut querier: MockQuerier<cosmwasm_std::Empty> = MockQuerier::new(&[]);
    querier.update_wasm(move |query| match query {
        WasmQuery::Smart {
            contract_addr: addr,
            msg,
        } => {
            if addr == contract_addr {
                let current = *count_ref.borrow();
                let requested: CoreQueryMsg = from_json(msg).unwrap();
                assert_eq!(requested, CoreQueryMsg::ExchangeRate {});
                let exchange_rate = exchange_rates[current];
                *count_ref.borrow_mut() += 1;
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&exchange_rate).unwrap()))
            } else {
                SystemResult::Err(SystemError::NoSuchContract { addr: addr.into() })
            }
        }
        _ => panic!("Unexpected query"),
    });

    let mut deps: OwnedDeps<_, _, MockQuerier, cosmwasm_std::Empty> = OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier,
        custom_query_type: std::marker::PhantomData,
    };
    let owner = deps.api.addr_make("owner");
    let core_contract = deps.api.addr_make("core_contract1");

    let deps_mut = deps.as_mut();

    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some(owner.as_ref())).unwrap();

    NEXT_HOUR_COUNTER.save(deps_mut.storage, &0).unwrap();

    CONFIG
        .save(
            deps_mut.storage,
            &Config {
                period_timeout: 10,
                core_contract,
                periods_to_keep: 2,
            },
        )
        .unwrap();

    let mut mocked_env = mock_env();
    mocked_env.block.time = Timestamp::from_seconds(60);
    mocked_env.block.height = 1;

    LAST_ER_UPDATE
        .save(deps_mut.storage, &Timestamp::from_seconds(0))
        .unwrap();

    execute_update_exchange_rates(deps_mut, mocked_env.clone(), message_info(&owner, &[])).unwrap();

    assert_eq!(NEXT_HOUR_COUNTER.load(deps.as_mut().storage).unwrap(), 1);
    assert_eq!(
        EXCHANGE_RATE.load(deps.as_mut().storage, 0).unwrap(),
        ExchangeRate {
            height: 1,
            exchange_rate: cloned_exchange_rates[0],
            timestamp: Timestamp::from_seconds(60),
        }
    );

    mocked_env.block.time = Timestamp::from_seconds(120);
    mocked_env.block.height = 10;

    execute_update_exchange_rates(deps.as_mut(), mocked_env.clone(), message_info(&owner, &[]))
        .unwrap();

    assert_eq!(NEXT_HOUR_COUNTER.load(deps.as_mut().storage).unwrap(), 2);

    assert_eq!(
        EXCHANGE_RATE.load(deps.as_mut().storage, 1).unwrap(),
        ExchangeRate {
            height: 10,
            exchange_rate: cloned_exchange_rates[1],
            timestamp: Timestamp::from_seconds(120),
        }
    );

    let response: Apy = from_json(
        query(
            deps.as_ref().into_empty(),
            mocked_env.clone(),
            QueryMsg::GetApy { time_span_hours: 1 },
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        response,
        Apy {
            start_exchange_rate: ExchangeRate {
                height: 1,
                exchange_rate: Decimal::from_str("1.01").unwrap(),
                timestamp: Timestamp::from_seconds(60),
            },
            end_exchange_rate: ExchangeRate {
                height: 10,
                exchange_rate: Decimal::from_str("1.02").unwrap(),
                timestamp: Timestamp::from_seconds(120),
            },
            apy: Decimal::from_str("5203.96039603960344").unwrap(),
        }
    );

    mocked_env.block.time = Timestamp::from_seconds(180);
    mocked_env.block.height = 20;

    execute_update_exchange_rates(deps.as_mut(), mocked_env.clone(), message_info(&owner, &[]))
        .unwrap();

    assert_eq!(NEXT_HOUR_COUNTER.load(deps.as_mut().storage).unwrap(), 3);

    assert_eq!(
        EXCHANGE_RATE
            .load(
                deps.as_mut().storage,
                2
            )
            .unwrap_err(),
        StdError::not_found("type: maxbtc_neutron_apy_calculator::state::ExchangeRate; key: [00, 0D, 65, 78, 63, 68, 61, 6E, 67, 65, 5F, 72, 61, 74, 65, 00, 00, 00, 00, 00, 00, 00, 02]".to_string())
    );

    assert_eq!(
        EXCHANGE_RATE
            .load(
                deps.as_mut().storage,
                0 // hour index should be zero because of `hours_to_keep` config option
            )
            .unwrap(),
        ExchangeRate {
            height: 20,
            exchange_rate: cloned_exchange_rates[2],
            timestamp: Timestamp::from_seconds(180),
        }
    );
}

#[test]
fn test_update_exchange_rate_five_hours() {
    let contract_addr = "cosmwasm1c6u47qaftrcxnsuszrnpj45z83v2sxxldl68h7f488g4s4mz50tsnmgg5l";

    let exchange_rates = vec![
        Decimal::from_str("1.01").unwrap(),
        Decimal::from_str("1.02").unwrap(),
        Decimal::from_str("1.03").unwrap(),
        Decimal::from_str("1.04").unwrap(),
        Decimal::from_str("1.05").unwrap(),
    ];
    let cloned_exchange_rates = exchange_rates.clone();

    let call_count = Rc::new(RefCell::new(0usize));
    let count_ref = call_count.clone();

    let mut querier: MockQuerier<cosmwasm_std::Empty> = MockQuerier::new(&[]);
    querier.update_wasm(move |query| match query {
        WasmQuery::Smart {
            contract_addr: addr,
            msg,
        } => {
            if addr == contract_addr {
                let current = *count_ref.borrow();
                let requested: CoreQueryMsg = from_json(msg).unwrap();
                assert_eq!(requested, CoreQueryMsg::ExchangeRate {});
                let exchange_rate = exchange_rates[current];
                *count_ref.borrow_mut() += 1;
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&exchange_rate).unwrap()))
            } else {
                SystemResult::Err(SystemError::NoSuchContract { addr: addr.into() })
            }
        }
        _ => panic!("Unexpected query"),
    });

    let mut deps: OwnedDeps<_, _, MockQuerier, cosmwasm_std::Empty> = OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier,
        custom_query_type: std::marker::PhantomData,
    };
    let owner = deps.api.addr_make("owner");
    let core_contract = deps.api.addr_make("core_contract1");

    let deps_mut = deps.as_mut();

    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some(owner.as_ref())).unwrap();

    NEXT_HOUR_COUNTER.save(deps_mut.storage, &0).unwrap();

    let period_timeout = 10;

    CONFIG
        .save(
            deps_mut.storage,
            &Config {
                period_timeout,
                core_contract,
                periods_to_keep: 5,
            },
        )
        .unwrap();

    let mut mocked_env = mock_env();
    mocked_env.block.time = Timestamp::from_seconds(60);

    LAST_ER_UPDATE
        .save(deps_mut.storage, &Timestamp::from_seconds(0))
        .unwrap();

    execute_update_exchange_rates(deps_mut, mocked_env.clone(), message_info(&owner, &[])).unwrap();

    assert_eq!(NEXT_HOUR_COUNTER.load(deps.as_mut().storage).unwrap(), 1);
    assert_eq!(
        EXCHANGE_RATE.load(deps.as_mut().storage, 0).unwrap(),
        ExchangeRate {
            height: 12345,
            exchange_rate: cloned_exchange_rates[0],
            timestamp: Timestamp::from_nanos(60000000000),
        }
    );

    mocked_env.block.time = mocked_env.block.time.plus_seconds(period_timeout);

    execute_update_exchange_rates(deps.as_mut(), mocked_env.clone(), message_info(&owner, &[]))
        .unwrap();

    assert_eq!(NEXT_HOUR_COUNTER.load(deps.as_mut().storage).unwrap(), 2);

    assert_eq!(
        EXCHANGE_RATE.load(deps.as_mut().storage, 1).unwrap(),
        ExchangeRate {
            height: 12345,
            exchange_rate: cloned_exchange_rates[1],
            timestamp: Timestamp::from_nanos(70000000000),
        }
    );

    mocked_env.block.time = mocked_env.block.time.plus_seconds(period_timeout);

    execute_update_exchange_rates(deps.as_mut(), mocked_env.clone(), message_info(&owner, &[]))
        .unwrap();

    assert_eq!(NEXT_HOUR_COUNTER.load(deps.as_mut().storage).unwrap(), 3);

    assert_eq!(
        EXCHANGE_RATE.load(deps.as_mut().storage, 2).unwrap(),
        ExchangeRate {
            height: 12345,
            exchange_rate: cloned_exchange_rates[2],
            timestamp: Timestamp::from_nanos(80000000000),
        }
    );

    mocked_env.block.time = mocked_env.block.time.plus_seconds(period_timeout);

    execute_update_exchange_rates(deps.as_mut(), mocked_env.clone(), message_info(&owner, &[]))
        .unwrap();

    assert_eq!(NEXT_HOUR_COUNTER.load(deps.as_mut().storage).unwrap(), 4);

    assert_eq!(
        EXCHANGE_RATE.load(deps.as_mut().storage, 3).unwrap(),
        ExchangeRate {
            height: 12345,
            exchange_rate: cloned_exchange_rates[3],
            timestamp: Timestamp::from_nanos(90000000000),
        }
    );

    mocked_env.block.time = mocked_env.block.time.plus_seconds(period_timeout);

    execute_update_exchange_rates(deps.as_mut(), mocked_env, message_info(&owner, &[])).unwrap();

    assert_eq!(NEXT_HOUR_COUNTER.load(deps.as_mut().storage).unwrap(), 5);

    assert_eq!(
        EXCHANGE_RATE.load(deps.as_mut().storage, 4).unwrap(),
        ExchangeRate {
            height: 12345,
            exchange_rate: cloned_exchange_rates[4],
            timestamp: Timestamp::from_nanos(100000000000),
        }
    );
}
