#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QuerierWrapper, QueryRequest, Reply, Response, StdResult, SubMsg, Uint128,
    WasmMsg, WasmQuery,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, CoreExecuteMsg, CoreQueryMsg::ExchangeRate, ExecuteMsg, InstantiateMsg,
    QueryMsg, StateResponse,
};
use crate::state::{Config, State, CONFIG, STATE};

const CONTRACT_NAME: &str = "crates.io:maxbtc-neutron-fee-collector";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MINT_FEE_REPLY_ID: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    if msg.fee_apy_reduction_percentage >= Decimal::one()
        || msg.fee_apy_reduction_percentage <= Decimal::zero()
    {
        return Err(ContractError::InvalidFeeReductionPercentage {});
    }

    let owner = deps.api.addr_validate(&msg.owner)?;
    let core_contract = deps.api.addr_validate(&msg.core_contract)?;

    let config = Config {
        owner,
        core_contract: core_contract.clone(),
        fee_apy_reduction_percentage: msg.fee_apy_reduction_percentage,
        collection_period_seconds: msg.collection_period_seconds,
        fee_denom: msg.fee_denom,
        maxbtc_decimals: msg.maxbtc_decimals,
    };
    CONFIG.save(deps.storage, &config)?;

    // Query the core contract for the initial exchange rate to bootstrap the state
    let initial_rate_response: Decimal = deps
        .querier
        .query_wasm_smart(core_contract, &ExchangeRate {})?;

    let state = State {
        last_collection_timestamp: env.block.time,
        last_exchange_rate: initial_rate_response,
    };
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("core_contract", msg.core_contract)
        .add_attribute("initial_exchange_rate", initial_rate_response.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CollectFee {} => execute_collect_fee(deps, env),
        ExecuteMsg::Claim { amount, recipient } => execute_claim(deps, info, amount, recipient),
        ExecuteMsg::UpdateConfig {
            owner,
            core_contract,
            fee_apy_reduction_percentage,
            collection_period_hours,
            maxbtc_decimals,
        } => execute_update_config(
            deps,
            env,
            info,
            owner,
            core_contract,
            fee_apy_reduction_percentage,
            collection_period_hours,
            maxbtc_decimals,
        ),
    }
}

pub fn execute_collect_fee(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // 1. Check if the collection period has passed
    let next_collection_time = state
        .last_collection_timestamp
        .plus_seconds(config.collection_period_seconds);
    if env.block.time < next_collection_time {
        return Err(ContractError::CollectionPeriodNotElapsed {});
    }

    // 2. Query current exchange rate from the core contract and total supply from the bank module
    let current_rate = query_exchange_rate(&deps.querier, &config.core_contract)?;
    let total_supply = deps.querier.query_supply(&config.fee_denom)?.amount;

    // 3. Check if APY is positive
    if current_rate <= state.last_exchange_rate {
        return Err(ContractError::NegativeOrZeroApy {
            current_rate,
            last_rate: state.last_exchange_rate,
        });
    }

    // 4. Calculate the amount of fee to mint
    let fee_to_mint = calculate_fee_to_mint(
        state.last_exchange_rate,
        current_rate,
        total_supply,
        config.fee_apy_reduction_percentage,
        config.maxbtc_decimals,
    )?;

    if fee_to_mint.is_zero() {
        // This case should theoretically be caught by the check above, but as a safeguard:
        return Ok(Response::new()
            .add_attribute("action", "collect_fee")
            .add_attribute("status", "no_fee_minted")
            .add_attribute("reason", "calculated_fee_was_zero"));
    }

    // 5. Create the MintFee message for the core contract
    let mint_msg = CoreExecuteMsg::MintFee {
        amount: Coin {
            denom: config.fee_denom,
            amount: fee_to_mint,
        },
    };

    let wasm_msg = WasmMsg::Execute {
        contract_addr: config.core_contract.to_string(),
        msg: to_json_binary(&mint_msg)?,
        funds: vec![],
    };

    // Use a submessage to handle the reply
    let sub_msg = SubMsg::reply_on_success(wasm_msg, MINT_FEE_REPLY_ID);

    Ok(Response::new()
        .add_submessage(sub_msg)
        .add_attribute("action", "collect_fee")
        .add_attribute("amount_to_mint", fee_to_mint.to_string()))
}

pub fn execute_claim(
    deps: DepsMut,
    info: MessageInfo,
    amount: Coin,
    recipient: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if amount.amount.is_zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let recipient_addr = deps.api.addr_validate(&recipient)?;

    let bank_send_msg = BankMsg::Send {
        to_address: recipient_addr.to_string(),
        amount: vec![amount.clone()],
    };

    Ok(Response::new()
        .add_message(CosmosMsg::Bank(bank_send_msg))
        .add_attribute("action", "claim")
        .add_attribute("recipient", recipient)
        .add_attribute("amount", amount.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    core_contract: Option<String>,
    fee_apy_reduction_percentage: Option<Decimal>,
    collection_period_hours: Option<u64>,
    maxbtc_decimals: Option<u32>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut response = Response::new().add_attribute("action", "update_config");

    if let Some(new_owner) = owner {
        config.owner = deps.api.addr_validate(&new_owner)?;
        response = response.add_attribute("owner_updated", new_owner);
    }
    if let Some(new_core_contract) = core_contract {
        config.core_contract = deps.api.addr_validate(&new_core_contract)?;
        response = response.add_attribute("core_contract_updated", new_core_contract.clone());
        let initial_rate_response: Decimal = deps
            .querier
            .query_wasm_smart(new_core_contract, &ExchangeRate {})?;
        let state = State {
            last_collection_timestamp: env.block.time,
            last_exchange_rate: initial_rate_response,
        };
        STATE.save(deps.storage, &state)?;
    }
    if let Some(new_percentage) = fee_apy_reduction_percentage {
        if new_percentage >= Decimal::one() || new_percentage <= Decimal::zero() {
            return Err(ContractError::InvalidFeeReductionPercentage {});
        }
        config.fee_apy_reduction_percentage = new_percentage;
        response = response.add_attribute(
            "fee_apy_reduction_percentage_updated",
            new_percentage.to_string(),
        );
    }
    if let Some(new_period) = collection_period_hours {
        config.collection_period_seconds = new_period * 60 * 60;
        response =
            response.add_attribute("collection_period_hours_updated", new_period.to_string());
    }
    if let Some(new_decimals) = maxbtc_decimals {
        config.maxbtc_decimals = new_decimals;
        response = response.add_attribute("maxbtc_decimals_updated", new_decimals.to_string());
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(response)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.id != MINT_FEE_REPLY_ID {
        return Err(ContractError::InvalidReplyId {});
    }

    let config = CONFIG.load(deps.storage)?;

    // Query the new exchange rate from the core contract
    let new_rate = query_exchange_rate(&deps.querier, &config.core_contract)?;

    // Update state with the new rate and timestamp
    let new_state = State {
        last_collection_timestamp: env.block.time,
        last_exchange_rate: new_rate,
    };
    STATE.save(deps.storage, &new_state)?;

    Ok(Response::new()
        .add_attribute("action", "reply_mint_fee")
        .add_attribute("new_exchange_rate", new_rate.to_string())
        .add_attribute("new_collection_timestamp", env.block.time.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::State {} => to_json_binary(&query_state(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        core_contract: config.core_contract.to_string(),
        fee_apy_reduction_percentage: config.fee_apy_reduction_percentage,
        collection_period_seconds: config.collection_period_seconds,
        fee_denom: config.fee_denom,
        maxbtc_decimals: config.maxbtc_decimals,
    })
}

fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {
        last_collection_timestamp: state.last_collection_timestamp,
        last_exchange_rate: state.last_exchange_rate,
    })
}

/// Converts a [`Decimal`] (which stores fixed-point numbers in *atomics*) back
/// into a concrete `Uint128` amount with the desired `decimals` precision.
pub fn dec_to_amount(dec: Decimal, decimals: u32) -> Result<Uint128, ContractError> {
    if dec.decimal_places() < decimals {
        // This should not happen if the decimal is constructed correctly
        return Err(ContractError::InvalidDecimalConversion {});
    }
    dec.atomics()
        .checked_div(Uint128::from(10u128.pow(dec.decimal_places() - decimals)))
        .map_err(ContractError::from)
}

/// Calculates the amount of fee tokens to mint to reduce the APY gain by a certain percentage.
pub fn calculate_fee_to_mint(
    rate_old: Decimal,
    rate_current: Decimal,
    total_supply_current: Uint128,
    fee_reduction_percentage: Decimal,
    maxbtc_decimals: u32,
) -> Result<Uint128, ContractError> {
    if rate_current <= rate_old {
        return Ok(Uint128::zero());
    }

    // Calculate the target exchange rate
    let gain = rate_current - rate_old;
    // rate_target = rate_old + gain * (1 - fee_reduction_percentage)
    let retained_gain_percentage = Decimal::one() - fee_reduction_percentage;
    let target_gain = gain * retained_gain_percentage;
    let rate_target = rate_old + target_gain;

    if rate_target.is_zero() {
        return Ok(Uint128::zero());
    }

    // Calculate the fee amount in Decimal form:
    // fee_amount = total_supply * (rate_current / rate_target - 1)
    // This calculates how much more valuable the token is at the current rate compared to the
    // target rate. For example, if current_rate is 1.1 and rate_target is 1.09, the rate_ratio is
    // ~1.00917. This means the total supply needs to be ~1.00917 times larger to bring the rate
    // down to 1.09.
    let rate_ratio = rate_current / rate_target;
    let factor = rate_ratio - Decimal::one();

    // Convert total supply from atomic Uint128 to a Decimal value
    let total_supply_dec = Decimal::from_atomics(total_supply_current, maxbtc_decimals)
        .map_err(|_| ContractError::InvalidDecimalConversion {})?;

    let fee_amount_decimal = factor * total_supply_dec;

    // Convert the fee amount from Decimal back to atomic Uint128
    dec_to_amount(fee_amount_decimal, maxbtc_decimals)
}

fn query_exchange_rate(querier: &QuerierWrapper, core_contract: &Addr) -> StdResult<Decimal> {
    let res: Decimal = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: core_contract.to_string(),
        msg: to_json_binary(&ExchangeRate {})?,
    }))?;
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_calculate_fee_to_mint() {
        // Case 1: Standard positive APY
        // Old rate: 1.0, Current rate: 1.1. Gain is 0.1.
        // Total supply: 1,000,000
        // Decimals: 6
        // Fee reduction: 10% (0.1)
        // We want to skim 10% of the 0.1 gain.
        // Retained gain should be 0.09. Target rate = 1.0 + 0.09 = 1.09.
        // total_supply_dec = 1.0
        // fee_dec = 1.0 * (1.1 / 1.09 - 1)
        // fee_dec = 1.0 * (0.0091743119...)
        // fee_dec = 0.0091743119...
        // Converting back to atomics (6 decimals) should give 9174
        let rate_old = Decimal::from_str("1.0").unwrap();
        let rate_current = Decimal::from_str("1.1").unwrap();
        let total_supply = Uint128::new(1_000_000); // 1.0 with 6 decimals
        let fee_percentage = Decimal::from_str("0.1").unwrap(); // 10%
        let decimals = 6;

        let fee = calculate_fee_to_mint(
            rate_old,
            rate_current,
            total_supply,
            fee_percentage,
            decimals,
        )
        .unwrap();
        assert_eq!(fee, Uint128::new(9174));

        // New total supply = 1,000,000 + 9174 = 1,009,174
        // Total assets (in value) = rate_current * old_supply_dec = 1.1 * 1.0 = 1.1
        // New rate = Total assets / new_supply_dec = 1.1 / 1.009174 = 1.0900002...
        // This is very close to the target of 1.09.

        // Case 2: No gain
        let rate_old_2 = Decimal::from_str("1.1").unwrap();
        let rate_current_2 = Decimal::from_str("1.1").unwrap();
        let fee_2 = calculate_fee_to_mint(
            rate_old_2,
            rate_current_2,
            total_supply,
            fee_percentage,
            decimals,
        )
        .unwrap();
        assert_eq!(fee_2, Uint128::zero());

        // Case 3: Negative APY (loss)
        let rate_old_3 = Decimal::from_str("1.1").unwrap();
        let rate_current_3 = Decimal::from_str("1.0").unwrap();
        let fee_3 = calculate_fee_to_mint(
            rate_old_3,
            rate_current_3,
            total_supply,
            fee_percentage,
            decimals,
        )
        .unwrap();
        assert_eq!(fee_3, Uint128::zero());

        // Case 4: Higher fee percentage (50%)
        // Target rate = 1.0 + (0.1 * 0.5) = 1.05
        // fee_dec = 1.0 * (1.1 / 1.05 - 1)
        // fee_dec = 1.0 * (0.047619...)
        // fee_dec = 0.047619...
        // Converting back to atomics (6 decimals) should give 47619
        let fee_percentage_4 = Decimal::from_str("0.5").unwrap(); // 50%
        let fee_4 = calculate_fee_to_mint(
            rate_old,
            rate_current,
            total_supply,
            fee_percentage_4,
            decimals,
        )
        .unwrap();
        assert_eq!(fee_4, Uint128::new(47619));
    }
}
