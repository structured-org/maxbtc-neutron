use crate::error::ContractError;
use cosmwasm_std::{Decimal, Uint128};

/// Converts a [`Decimal`] (which stores fixed-point numbers in *atomics*) back
/// into a concrete `Uint128` amount with the desired `decimals` precision.
pub fn dec_to_amount(dec: Decimal, decimals: u32) -> Result<Uint128, ContractError> {
    dec.atomics()
        .checked_div(Uint128::from(10u128.pow(dec.decimal_places() - decimals)))
        .map_err(ContractError::DivideByZeroError)
}
