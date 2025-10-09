use cosmwasm_std::StdError;
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

    // #[error("{0}")]
    // DecimalRangeExceeded(#[from] DecimalRangeExceeded),

    // #[error("{0}")]
    // OverflowError(#[from] OverflowError),
    #[error("{0}")]
    PaymentError(#[from] PaymentError),

    #[error("Invalid deposit amount")]
    InvalidDepositAmount {},
}
