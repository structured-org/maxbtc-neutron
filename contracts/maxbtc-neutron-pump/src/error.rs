use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Fee amount exceeds the maximum allowed fee")]
    InvalidFee {},

    #[error("Oracle address mismatch: The address provided by the oracle does not match the configured address")]
    OracleMismatch {},

    #[error("Invalid reply ID")]
    InvalidReplyID {},
}
