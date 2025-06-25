use crate::error::AppError;
use dotenv::dotenv;
use std::env;

#[derive(Clone)]
pub struct Config {
    pub execute_period: u64,
    pub contract_address: String,
    pub grpc_address: String,
    pub grpc_port: String,
    pub mnemonic: String,
    pub denom: String,
    pub eureka_full_timeout_multiplier: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        dotenv().ok();

        let execute_period = env::var("EXECUTE_PERIOD")
            .map_err(|_| AppError::Config("EXECUTE_PERIOD not set".to_string()))?
            .parse::<u64>()
            .map_err(|_| AppError::Config("EXECUTE_PERIOD must be a number".to_string()))?;

        let contract_address = env::var("CONTRACT_ADDRESS")
            .map_err(|_| AppError::Config("CONTRACT_ADDRESS not set".to_string()))?;

        let grpc_address = env::var("GRPC_ADDRESS")
            .map_err(|_| AppError::Config("RPC_ADDRESS not set".to_string()))?;

        let grpc_port = env::var("GRPC_PORT")
            .map_err(|_| AppError::Config("RPC_PORT not set".to_string()))?;

        let mnemonic =
            env::var("MNEMONIC").map_err(|_| AppError::Config("MNEMONIC not set".to_string()))?;

        let denom = env::var("DENOM").map_err(|_| AppError::Config("DENOM not set".to_string()))?;

        let eureka_full_timeout_multiplier = env::var("EUREKA_FULL_TIMEOUT_MULTIPLIER")
            .map_err(|_| AppError::Config("EUREKA_FULL_TIMEOUT_MULTIPLIER not set".to_string()))?
            .parse::<u64>()
            .map_err(|_| {
                AppError::Config("EUREKA_FULL_TIMEOUT_MULTIPLIER must be a number".to_string())
            })?;

        Ok(Self {
            execute_period,
            contract_address,
            grpc_address,
            grpc_port,
            mnemonic,
            denom,
            eureka_full_timeout_multiplier,
        })
    }
}
