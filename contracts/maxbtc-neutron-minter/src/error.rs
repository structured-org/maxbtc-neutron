use cosmwasm_std::{DecimalRangeExceeded, DivideByZeroError, OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Contract is paused")]
    ContractPaused {},

    #[error("No funds sent in deposit")]
    NoFundsSent {},

    #[error("Invalid deposit denom. Expected {expected}, got {received}")]
    InvalidDepositDenom { expected: String, received: String },

    #[error("Invalid deposit amount")]
    InvalidDepositAmount {},

    #[error("Invalid withdraw amount")]
    InvalidWithdrawAmount {},

    #[error("Batch not found or in incorrect state")]
    BatchStateError {},

    #[error("Batch not in FINALIZED state")]
    BatchNotFinalized {},

    #[error("Cannot process active batch yet (still within active duration)")]
    CannotProcessActiveBatchYet {},

    #[error("Cannot finalize withdrawing batch yet (still within withdrawing duration)")]
    CannotFinalizeWithdrawingBatchYet {},

    #[error("Protocol is in emergency paused state. Manual intervention required.")]
    ProtocolInEmergency {},

    #[error("Redemption token supply mismatch")]
    RedemptionSupplyMismatch {},

    #[error("Wrong redemption token denom or no redemption tokens attached")]
    WrongRedemptionTokenOrNoFunds {},

    #[error("{0}")]
    DecimalRangeExceeded(#[from] DecimalRangeExceeded),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("Deposit cap was exceeded")]
    DepositCapExceeded {},

    #[error("{0}")]
    DivideByZeroError(#[from] DivideByZeroError),
}
