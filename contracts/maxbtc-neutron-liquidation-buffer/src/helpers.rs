use crate::error::ContractError;
#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    BalanceResponse, BankMsg, BankQuery, Coin, CosmosMsg, Decimal, Deps,
    Env, QueryRequest, Uint128
};

use neutron_std::types::neutron::dex::{
    DexQuerier, MsgCancelLimitOrder,
    QueryAllLimitOrderTrancheUserByAddressResponse,
};
use neutron_std::types::osmosis::tokenfactory::v1beta1::MsgBurn;
/// Queries the contract's balance for the specified token denom. [BTC, MAXBTC]
pub fn get_pure_contract_balance(
    deps: Deps,
    env: Env,
    btc_denom: String,
    maxbtc_denom: String,
) -> Result<Vec<Coin>, ContractError> {
    let mut balances = vec![];
    for denom in &[btc_denom, maxbtc_denom] {
        let balance_request = QueryRequest::Bank(BankQuery::Balance {
            address: env.contract.address.to_string(),
            denom: denom.clone(),
        });

        // Query the balance for each denom
        let balance_resp: BalanceResponse = deps.querier.query(&balance_request)?;

        // Add the balance to the balances vector
        balances.push(Coin {
            denom: denom.clone(),
            amount: balance_resp.amount.amount,
        });
    }

    Ok(balances)
}

/// Get the virtual contract balance. Which includes all the tokens deposited in AMM positions + the tokens available in the contract.
pub fn get_virtual_contract_balance(
    env: Env,
    deps: Deps,
    btc_denom: String,
    maxbtc_denom: String,
) -> Result<(Uint128, Uint128), ContractError> {
    let dex_querier = DexQuerier::new(&deps.querier);
    // simulate full withdrawal to get the current total token amounts:
    let res: QueryAllLimitOrderTrancheUserByAddressResponse = dex_querier
        .limit_order_tranche_user_all_by_address(env.contract.address.to_string(), None)?;
    // If there are any active deposits, withdraw all of them

    let balances =
        get_pure_contract_balance(deps, env.clone(), btc_denom.clone(), maxbtc_denom.clone())?;
    let mut total_amount_btc = balances[0].amount;
    let mut total_amount_maxbtc = balances[1].amount;

    for order in res.limit_orders.iter() {
        let cancel_msg = MsgCancelLimitOrder {
            creator: env.contract.address.to_string(),
            tranche_key: order.tranche_key.clone(),
        };

        let sim_response = dex_querier.simulate_cancel_limit_order(Some(cancel_msg))?;

        if let Some(resp) = sim_response.resp {
            // Only BTC -> MAXBTC orders exist
            // taker_coin_out is MAXBTC, maker_coin_out is BTC
            if let Some(taker_coin) = resp.taker_coin_out {
                if taker_coin.denom == maxbtc_denom {
                    total_amount_maxbtc += taker_coin.amount.parse::<Uint128>().unwrap_or_default();
                } else if taker_coin.denom == btc_denom {
                    total_amount_btc += taker_coin.amount.parse::<Uint128>().unwrap_or_default();
                }
            }
            if let Some(maker_coin) = resp.maker_coin_out {
                if maker_coin.denom == btc_denom {
                    total_amount_btc += maker_coin.amount.parse::<Uint128>().unwrap_or_default();
                } else if maker_coin.denom == maxbtc_denom {
                    total_amount_maxbtc += maker_coin.amount.parse::<Uint128>().unwrap_or_default();
                }
            }
        }
    }
    Ok((total_amount_btc, total_amount_maxbtc))
}

/// Returns the cancel messages, the total BTC the contract holds after cancelling, and the total MAXBTC the contract holds after cancelling.
/// If no cancel messages are returned, the contract holds the full amount of BTC and MAXBTC and that is returned with an empty vector.
pub fn get_cancel_messages(
    env: Env,
    deps: Deps,
    btc_denom: String,
    maxbtc_denom: String,
) -> Result<(Vec<CosmosMsg>, Uint128, Uint128), ContractError> {
    let dex_querier = DexQuerier::new(&deps.querier);
    // get the current balance of the contract
    let balances =
        get_pure_contract_balance(deps, env.clone(), btc_denom.clone(), maxbtc_denom.clone())?;
    let mut total_amount_btc = balances[0].amount;
    let mut total_amount_maxbtc = balances[1].amount;

    // get all limit orders created by this contract
    let res: QueryAllLimitOrderTrancheUserByAddressResponse = dex_querier
        .limit_order_tranche_user_all_by_address(env.contract.address.to_string(), None)?;

    // create a vector of cancel messages for all limit orders
    let mut cancel_msgs = vec![];
    // iterate over all limit orders, if any are found, create a cancel message
    // for each while adding the expected tokens to the total balance
    for order in res.limit_orders.iter() {
        let cancel_msg = MsgCancelLimitOrder {
            creator: env.contract.address.to_string(),
            tranche_key: order.tranche_key.clone(),
        };

        cancel_msgs.push(cancel_msg.clone().into());

        // simulate the cancel message to get the expected tokens returned to the contract
        let sim_response = dex_querier.simulate_cancel_limit_order(Some(cancel_msg))?;

        if let Some(resp) = sim_response.resp {
            // taker_coin_out is MAXBTC, maker_coin_out is BTC
            if let Some(taker_coin) = resp.taker_coin_out {
                // add the output amount to the total balance
                // taker coin is MAXBTC, maker coin is BTC
                if taker_coin.denom == maxbtc_denom {
                    total_amount_maxbtc += taker_coin.amount.parse::<Uint128>().unwrap_or_default();
                }
            }
            if let Some(maker_coin) = resp.maker_coin_out {
                // add the coin_out to the total balance
                if maker_coin.denom == btc_denom {
                    total_amount_btc += maker_coin.amount.parse::<Uint128>().unwrap_or_default();
                }
            }
        }
    }
    Ok((cancel_msgs, total_amount_btc, total_amount_maxbtc))
}

/// Get the deposit price. which is the exchange rate multiplied by the price multiplier as a percentage
pub fn get_deposit_price(
    exchange_rate: Decimal,
    price_multiplier_basis_points: u128,
) -> Result<Decimal, ContractError> {
    // convert the price multiplier to a percentage
    let percentage_surcharge = Decimal::from_atomics(price_multiplier_basis_points, 0)
        .map_err(|_| ContractError::Unauthorized {})?
        .checked_div(Decimal::from_atomics(10000u128, 0).unwrap())
        .map_err(|_| ContractError::Unauthorized {})?;

    // calculate the deposit price by multiplying the exchange rate by the multiplier percentage
    let deposit_price = exchange_rate
        .checked_mul(
            Decimal::one()
                .checked_add(percentage_surcharge)
                .map_err(|_| ContractError::Unauthorized {})?,
        )
        .map_err(|_| ContractError::Unauthorized {})?;

    Ok(deposit_price)
}
/// Burns most of the maxBTC and returns some maxBTC to the treasury.
pub fn get_burn_and_refund_messages(
    env: Env,
    max_btc_amount: Uint128,
    original_price: Decimal,
    new_price: Decimal,
    beneficiary: String,
    max_btc_denom: String,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let mut messages = vec![];

    // new price should be always greater than original price, since mutiplier is always greater than 1.
    // price divergence is how much greater the new price is from the original price as a percentage.
    // eg: 
    // exvhange rate= 1.1
    //price_multiplier_basis_points = 350 (3.5% surcharge)
    // new price = 1.1385 (3.5% surcharge)
    // price divergence = 1.1 / 1.1385 = 0.96618357. 96.618357% should be burned and 3.381643% should be returned.
    let price_divergance = original_price.checked_div(new_price).map_err(|_| ContractError::Unauthorized {})?;

    // the burn amount is floored
    let burn_amount = Decimal::from_atomics(max_btc_amount, 0).map_err(|_| ContractError::Unauthorized {})?
        .checked_mul(price_divergance)
        .map_err(|_| ContractError::Unauthorized {})?
        .to_uint_floor();

    // since the burn amout is floored, the rouding error will be in favor of the treasury return_amount.
    let return_amount = max_btc_amount.checked_sub(burn_amount).map_err(|_| ContractError::Unauthorized {})?;

    messages.push(
        MsgBurn {
            sender: env.contract.address.to_string(),
            amount: Some(
                Coin {
                    denom: max_btc_denom.clone(),
                    amount: burn_amount,
                }
                .into(),
            ),
            burn_from_address: env.contract.address.to_string(),
        }
        .into(),
    );

    messages.push(
        BankMsg::Send {
            to_address: beneficiary.clone(),
            amount: vec![Coin {
                denom: max_btc_denom.clone(),
                amount: return_amount,
            }],
        }
        .into(),
    );

    Ok(messages)
}
