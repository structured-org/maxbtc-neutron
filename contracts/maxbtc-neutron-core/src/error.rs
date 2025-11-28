use cosmwasm_std::{
    DecimalRangeExceeded, DivideByZeroError, Instantiate2AddressError, OverflowError, StdError,
};
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

    #[error("Contract is paused")]
    ContractPaused {},

    #[error("No funds sent in deposit")]
    NoFundsSent {},

    #[error("Invalid deposit denom. Expected {expected}, got {received}")]
    InvalidDepositDenom { expected: String, received: String },

    #[error("Invalid fee denom. Expected {expected}, got {received}")]
    InvalidFeeDenom { expected: String, received: String },

    #[error("Invalid deposit amount")]
    InvalidDepositAmount {},

    #[error("Invalid withdraw amount")]
    InvalidWithdrawAmount {},

    #[error("Recipient address not allowed to mint maxBTC")]
    AddressNotAllowed {},

    #[error("Batch not found or in incorrect state")]
    BatchStateError {},

    #[error("Batch not in FINALIZED state")]
    BatchNotFinalized {},

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

    #[error("Slippage limit exceeded. Requested at least {requested}, actual {actual}")]
    SlippageLimitExceeded { requested: u128, actual: u128 },

    #[error("{0}")]
    DivideByZeroError(#[from] DivideByZeroError),

    #[error("{0}")]
    Instantiate2Error(Instantiate2AddressError),

    #[error("Flush deposit is allowed in DepositNeutron state only")]
    FlushDepositAllowedInDepositNeutron {},

    #[error("Waitosaur is locked")]
    WaitosaurLocked {},

    #[error("{0}")]
    PaymentError(#[from] PaymentError),

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
