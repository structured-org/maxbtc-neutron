use cosmwasm_std::{
    attr, entry_point, to_json_binary, Attribute, Binary, Decimal, Deps, DepsMut, Env, Event,
    MessageInfo, Response, StdResult, Timestamp, Uint128,
};

use crate::{
    error::ContractError,
    msgs::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    state::{
        Apy, Config, CoreQueryMsg, ExchangeRate, CONFIG, EXCHANGE_RATE, LAST_ER_UPDATE,
        NEXT_HOUR_COUNTER,
    },
};

const CONTRACT_NAME: &str = "maxbtc-neutron-apy-calculator";
const CONTRACT_VERSION: &str = "0.1.0";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let attrs = vec![attr("msg", format!("{:?}", msg))];
    cw_ownable::initialize_owner(deps.storage, deps.api, Some(msg.owner.as_str()))?;
    CONFIG.save(
        deps.storage,
        &Config {
            core_contract: deps.api.addr_validate(msg.core_contract.as_str())?,
            periods_to_keep: msg.periods_to_keep,
            period_timeout: msg.period_timeout,
        },
    )?;

    NEXT_HOUR_COUNTER.save(deps.storage, &0)?;
    LAST_ER_UPDATE.save(deps.storage, &Timestamp::from_seconds(0))?;

    Ok(response("instantiate", CONTRACT_NAME, attrs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetApy { time_span_hours } => query_apy(deps, env, time_span_hours),
        QueryMsg::GetConfig {} => query_config(deps),
        QueryMsg::Ownership {} => {
            let ownership = cw_ownable::get_ownership(deps.storage)?;

            Ok(to_json_binary(&ownership)?)
        }
    }
}

fn query_apy(deps: Deps, _env: Env, time_span_hours: u64) -> Result<Binary, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if time_span_hours >= config.periods_to_keep {
        return Err(ContractError::TimespanOutOfRange {});
    }

    let next_hour_counter = NEXT_HOUR_COUNTER.load(deps.storage)?;
    let end_hour_counter = next_hour_counter - 1;

    let end_exchange_rate =
        EXCHANGE_RATE.load(deps.storage, end_hour_counter % config.periods_to_keep)?;

    let mut start_hour_counter = end_hour_counter.saturating_sub(time_span_hours);

    while !EXCHANGE_RATE.has(deps.storage, start_hour_counter % config.periods_to_keep) {
        start_hour_counter += 1;
    }

    let start_exchange_rate =
        EXCHANGE_RATE.load(deps.storage, start_hour_counter % config.periods_to_keep)?;

    let apy = predict_apy(&start_exchange_rate, &end_exchange_rate)?;

    Ok(to_json_binary(&Apy {
        start_exchange_rate,
        end_exchange_rate,
        apy,
    })?)
}

pub fn predict_apy(start: &ExchangeRate, end: &ExchangeRate) -> Result<Decimal, ContractError> {
    if end.timestamp <= start.timestamp {
        return Err(ContractError::EndBeforeStart);
    }

    let dt_sec = Uint128::from((end.timestamp.seconds() - start.timestamp.seconds()) as u128);
    let rate_start = start.exchange_rate;
    let rate_end = end.exchange_rate;

    let delta = (rate_end - rate_start) / rate_start;

    let sec_per_year = Uint128::from((365 * 24 * 60 * 60) as u128);

    let apy = delta * Decimal::from_ratio(sec_per_year, dt_sec);

    Ok(apy)
}

fn query_config(deps: Deps) -> Result<Binary, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    Ok(to_json_binary(&config)?)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateExchangeRates {} => execute_update_exchange_rates(deps, env, info),
        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(response::<(&str, &str), _>(
                "execute-update-ownership",
                CONTRACT_NAME,
                [],
            ))
        }
    }
}

pub fn execute_update_exchange_rates(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let last_er_update = LAST_ER_UPDATE.load(deps.storage)?;
    if last_er_update
        .plus_seconds(config.period_timeout)
        .gt(&env.block.time)
    {
        return Err(ContractError::TooEarly {});
    }
    LAST_ER_UPDATE.save(deps.storage, &env.block.time)?;

    let next_hour_counter = NEXT_HOUR_COUNTER.load(deps.storage)?;
    NEXT_HOUR_COUNTER.save(deps.storage, &(next_hour_counter + 1))?;

    let mut attrs = vec![attr("action", "update_exchange_rates")];
    attrs.push(attr("next_hour_idx", next_hour_counter.to_string()));

    let exchange_rate: Decimal = deps
        .querier
        .query_wasm_smart(config.core_contract.clone(), &CoreQueryMsg::ExchangeRate {})?;

    attrs.push(attr("exchange_rate", exchange_rate.to_string()));

    EXCHANGE_RATE.save(
        deps.storage,
        next_hour_counter % config.periods_to_keep,
        &ExchangeRate {
            height: env.block.height,
            timestamp: env.block.time,
            exchange_rate,
        },
    )?;

    Ok(response("update_exchange_rates", CONTRACT_NAME, attrs))
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::new())
}

pub fn response<A: Into<Attribute>, T>(
    ty: &str,
    contract_name: &str,
    attrs: impl IntoIterator<Item = A>,
) -> Response<T> {
    Response::<T>::new()
        .add_event(Event::new(format!("{}-{}", contract_name, ty)).add_attributes(attrs))
}
