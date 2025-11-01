use cosmwasm_std::{Instantiate2AddressError, StdError};
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OwnershipError(#[from] cw_ownable::OwnershipError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("{0}")]
    PaymentError(#[from] PaymentError),

    #[error("Invalid deposit amount")]
    InvalidDepositAmount {},

    #[error("Semver parsing error: {0}")]
    SemVer(String),

    #[error("Could not calculcate instantiate2 address: {0}")]
    Instantiate2AddressError(#[from] Instantiate2AddressError),

    #[error("Wrong funds attached")]
    WrongFundsAttached {},
}

impl From<semver::Error> for ContractError {
    fn from(err: semver::Error) -> Self {
        Self::SemVer(err.to_string())
    }
}
