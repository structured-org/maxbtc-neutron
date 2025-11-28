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

    #[error("Collection period must not be less than one hour")]
    InvalidCollectionPeriod {},

    #[error("Can't migrate from {storage_contract_name} to {contract_name}")]
    MigrationError {
        storage_contract_name: String,
        contract_name: String,
    },

    #[error("Semver parsing error: {0}")]
    SemVer(String),
}

impl From<semver::Error> for ContractError {
    fn from(err: semver::Error) -> Self {
        Self::SemVer(err.to_string())
    }
}
