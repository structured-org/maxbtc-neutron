use crate::error::ContractError;
use cosmwasm_std::{Coin, Decimal, Uint128};

/// Converts a [`Decimal`] (which stores fixed-point numbers in *atomics*) back
/// into a concrete `Uint128` amount with the desired `decimals` precision.
pub fn dec_to_amount(dec: Decimal, decimals: u32) -> Result<Uint128, ContractError> {
    dec.atomics()
        .checked_div(Uint128::from(10u128.pow(dec.decimal_places() - decimals)))
        .map_err(ContractError::DivideByZeroError)
}

/// Extracts and validates the *deposit coin* from the `funds` vector passed to
/// an `execute` entry point.
///
/// The function enforces:
///
/// * Exactly **one** coin must be supplied.
/// * That coin must use the configured `deposit_denom`.
/// * The amount must be **non-zero**.
pub fn get_deposit_coin(deposit_denom: String, funds: Vec<Coin>) -> Result<Coin, ContractError> {
    if funds.is_empty() {
        return Err(ContractError::NoFundsSent {});
    }
    if funds.len() != 1 {
        return Err(ContractError::InvalidDepositAmount {});
    }
    let deposit_coin = funds[0].clone();
    if deposit_coin.denom != deposit_denom {
        return Err(ContractError::InvalidDepositDenom {
            expected: deposit_denom,
            received: deposit_coin.denom.clone(),
        });
    }
    if deposit_coin.amount.is_zero() {
        return Err(ContractError::InvalidDepositAmount {});
    }

    Ok(deposit_coin)
}
