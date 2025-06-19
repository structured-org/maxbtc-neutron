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

    #[error("Oracle address mismatch: {value}")]
    OracleMismatch { value: String },

    #[error("Invalid reply ID")]
    InvalidReplyID {},
}
