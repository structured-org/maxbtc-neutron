use crate::error::AppError;
use dotenv::dotenv;
use std::env;
use std::str::FromStr;

#[derive(Clone)]
pub struct Config {
    pub execute_period: u64,
    pub contract_address: String,
    pub grpc_address: String,
    pub grpc_port: String,
    pub mnemonic: String,
    // The IBC denom of the coin on Neutron (the coin we transfer)
    pub neutron_denom: String,
    // The IBC denom of the coin on Cosmos Hub (the coin we pay the fee in, that's
    // what Skip API gives us); it's the same coin, but on Cosmos Hub
    pub hub_denom: String,

    // New Skip API Config
    pub skip_api_url: String,
    pub source_asset_chain_id: String,
    pub dest_asset_denom: String,
    pub dest_asset_chain_id: String,
    pub allow_multi_tx: bool,
    pub allow_unsafe: bool,
    pub go_fast: bool,
    pub smart_relay: bool,
    pub split_routes: bool,
    pub evm_swaps: bool,
    pub experimental_features: Vec<String>,
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
            .map_err(|_| AppError::Config("GRPC_ADDRESS not set".to_string()))?;

        let grpc_port =
            env::var("GRPC_PORT").map_err(|_| AppError::Config("GRPC_PORT not set".to_string()))?;

        let mnemonic =
            env::var("MNEMONIC").map_err(|_| AppError::Config("MNEMONIC not set".to_string()))?;

        let neutron_denom = env::var("NEUTRON_DENOM")
            .map_err(|_| AppError::Config("NEUTRON_DENOM not set".to_string()))?;
        let hub_denom =
            env::var("HUB_DENOM").map_err(|_| AppError::Config("HUB_DENOM not set".to_string()))?;

        // Load Skip API configuration
        let skip_api_url = env::var("SKIP_API_URL")
            .map_err(|_| AppError::Config("SKIP_API_URL not set".to_string()))?;

        let source_asset_chain_id = env::var("SOURCE_ASSET_CHAIN_ID")
            .map_err(|_| AppError::Config("SOURCE_ASSET_CHAIN_ID not set".to_string()))?;

        let dest_asset_denom = env::var("DEST_ASSET_DENOM")
            .map_err(|_| AppError::Config("DEST_ASSET_DENOM not set".to_string()))?;

        let dest_asset_chain_id = env::var("DEST_ASSET_CHAIN_ID")
            .map_err(|_| AppError::Config("DEST_ASSET_CHAIN_ID not set".to_string()))?;

        // Helper to parse bools
        let parse_bool = |var_name: &str| -> Result<bool, AppError> {
            let val_str =
                env::var(var_name).map_err(|_| AppError::Config(format!("{var_name} not set")))?;
            bool::from_str(&val_str)
                .map_err(|_| AppError::Config(format!("{var_name} must be 'true' or 'false'")))
        };

        let allow_multi_tx = parse_bool("SKIP_ALLOW_MULTI_TX")?;
        let allow_unsafe = parse_bool("SKIP_ALLOW_UNSAFE")?;
        let go_fast = parse_bool("SKIP_GO_FAST")?;
        let smart_relay = parse_bool("SKIP_SMART_RELAY")?;
        let split_routes = parse_bool("SKIP_SPLIT_ROUTES")?;
        let evm_swaps = parse_bool("SKIP_EVM_SWAPS")?;

        let experimental_features_str = env::var("SKIP_EXPERIMENTAL_FEATURES").unwrap_or_default();
        let experimental_features = if experimental_features_str.is_empty() {
            vec![]
        } else {
            experimental_features_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };

        Ok(Self {
            execute_period,
            contract_address,
            grpc_address,
            grpc_port,
            mnemonic,
            neutron_denom,
            hub_denom,
            skip_api_url,
            source_asset_chain_id,
            dest_asset_denom,
            dest_asset_chain_id,
            allow_multi_tx,
            allow_unsafe,
            go_fast,
            smart_relay,
            split_routes,
            evm_swaps,
            experimental_features,
        })
    }
}
