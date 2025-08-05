use cosmwasm_std::StdError;
use cw_ownable::OwnershipError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),
    #[error("Contract doesn't have enough funds")]
    InsufficientFunds,
    #[error("Target balance doesn't exist")]
    UnknownTargetBalance,
    #[error("{0}")]
    OwnershipError(#[from] OwnershipError),
    #[error("Period end is before period start")]
    EndBeforeStart,
    #[error("Too early to update exchange rates")]
    TooEarly {},
    #[error("Timespan is out of range")]
    TimespanOutOfRange {},
}
