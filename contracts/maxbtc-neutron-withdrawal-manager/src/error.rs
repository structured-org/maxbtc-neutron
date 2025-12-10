use cosmwasm_std::{DecimalRangeExceeded, DivideByZeroError, OverflowError, StdError};
use cw_ownable::OwnershipError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Batch is not withdrawn yet")]
    BatchIsNotWithdrawn {},

    #[error("Missing unbonded amount in batch")]
    BatchAmountIsEmpty {},

    #[error("Redemption token supply mismatch")]
    RedemptionSupplyMismatch {},

    #[error("Wrong redemption token denom or no redemption tokens attached")]
    WrongRedemptionTokenOrNoFunds {},

    #[error("{0}")]
    OwnershipError(#[from] OwnershipError),

    #[error("Contract is paused")]
    ContractPaused {},

    #[error("Semver parsing error: {0}")]
    SemVer(String),

    #[error("Can't migrate from {storage_contract_name} to {contract_name}")]
    MigrationError {
        storage_contract_name: String,
        contract_name: String,
    },

    #[error("{0}")]
    DivideByZeroError(#[from] DivideByZeroError),

    #[error("{0}")]
    DecimalRangeExceeded(#[from] DecimalRangeExceeded),
}

impl From<semver::Error> for ContractError {
    fn from(err: semver::Error) -> Self {
        Self::SemVer(err.to_string())
    }
}
