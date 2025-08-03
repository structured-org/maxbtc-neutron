use cosmwasm_std::{Decimal, DivideByZeroError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized: sender is not the contract owner")]
    Unauthorized {},

    #[error("Invalid recipient address")]
    InvalidRecipient {},

    #[error("Fee collection is not allowed yet. The cooldown period has not passed.")]
    CollectionPeriodNotElapsed {},

    #[error("No profit to collect: APY is not positive")]
    NegativeOrZeroApy {
        current_rate: Decimal,
        last_rate: Decimal,
    },

    #[error("Invalid reply ID")]
    InvalidReplyId {},

    #[error("Invalid zero amount")]
    InvalidZeroAmount {},

    #[error("Invalid fee reduction percentage: must be between 0 and 1")]
    InvalidFeeReductionPercentage {},

    #[error("Failed to convert value to Decimal")]
    InvalidDecimalConversion {},

    #[error("Division by zero error")]
    DivideByZeroError(#[from] DivideByZeroError),
}
