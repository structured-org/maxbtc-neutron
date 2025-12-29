use cosmwasm_std::{DivideByZeroError, Instantiate2AddressError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("{0}")]
    DivideByZeroError(#[from] DivideByZeroError),
    #[error("{0}")]
    Instantiate2Error(Instantiate2AddressError),
}

pub type ContractResult<T> = Result<T, ContractError>;
