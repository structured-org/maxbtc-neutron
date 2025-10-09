use cosmwasm_std::StdError;
use cw_ownable::OwnershipError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("{0}")]
    Ownable(#[from] OwnershipError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Already Locked")]
    AlreadyLocked {},

    #[error("Already Unlocked")]
    AlreadyUnlocked {},

    #[error("No data available in the target contract")]
    NoDataInContract {},

    #[error("No asset found in the published data")]
    NoAssetFound {},

    #[error("Insufficient asset amount to unlock")]
    InsufficientAssetAmount {},

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
