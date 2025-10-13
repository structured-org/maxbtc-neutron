use crate::config::Config;
use crate::error::AppError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::Coin;
use valence_domain_clients::clients::neutron::NeutronClient;
use valence_domain_clients::cosmos::base_client::BaseClient;
use valence_domain_clients::cosmos::wasm_client::WasmClient;

#[cw_serde]
pub enum ExecuteMsg {
    IbcTransfer {},
    EurekaTransfer { eureka_fee: EurekaFee },
}

#[cw_serde]
pub struct EurekaFee {
    pub coin: Coin,
    pub receiver: String,
    // In nanoseconds
    pub timeout_timestamp: u64,
}

pub struct ChainClient {
    client: NeutronClient,
}

impl ChainClient {
    pub async fn new(config: &Config) -> Result<Self, AppError> {
        let client = NeutronClient::new(
            &config.grpc_address,
            &config.grpc_port,
            &config.mnemonic,
            "neutron-1",
        )
        .await
        .map_err(|e| AppError::Chain(e.to_string()))?;
        Ok(Self { client })
    }

    pub async fn query_balance(&self, address: &str, denom: &str) -> Result<u128, AppError> {
        self.client
            .query_balance(address, denom)
            .await
            .map_err(|e| AppError::Chain(e.to_string()))
    }

    pub async fn execute_message(
        &self,
        contract_address: &str,
        msg: ExecuteMsg,
    ) -> Result<String, AppError> {
        log::info!("Executing wasm message on contract: {contract_address}");
        log::debug!("Push message: {msg:?}");
        let tx_resp = self
            .client
            .execute_wasm(contract_address, msg, vec![], None)
            .await
            .map_err(|e| AppError::Chain(e.to_string()))?;

        log::info!("Transaction sent with hash: {}", tx_resp.hash);

        let tx_result = self
            .client
            .poll_for_tx(&tx_resp.hash)
            .await
            .map_err(|e| AppError::Chain(e.to_string()))?;

        if tx_result.code == 0 {
            log::info!("Transaction successful!");
        } else {
            log::error!("Transaction failed: {tx_result:?}");
        }

        Ok(tx_resp.hash)
    }
}
