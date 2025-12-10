use crate::config::Config; // Import the Config struct
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct SkipRouteRequest {
    pub source_asset_denom: String,
    pub source_asset_chain_id: String,
    pub dest_asset_denom: String,
    pub dest_asset_chain_id: String,
    pub amount_in: String,
    pub allow_multi_tx: bool,
    pub allow_unsafe: bool,
    pub go_fast: bool,
    pub smart_relay: bool,
    pub experimental_features: Vec<String>,
    pub smart_swap_options: SmartSwapOptions,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SmartSwapOptions {
    pub split_routes: bool,
    pub evm_swaps: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SkipRouteResponse {
    pub operations: Vec<Operation>,
    pub estimated_route_duration_seconds: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Operation {
    #[serde(rename = "eureka_transfer")]
    pub eureka_transfer: Option<EurekaTransfer>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EurekaTransfer {
    #[serde(rename = "source_client")]
    pub source_client: String,
    #[serde(rename = "entry_contract_address")]
    pub to_chain_entry_contract_address: String,
    #[serde(rename = "callback_adapter_contract_address")]
    pub to_chain_callback_contract_address: String,
    #[serde(rename = "smart_relay_fee_quote")]
    pub smart_relay_fee_quote: SmartRelayFeeQuote,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SmartRelayFeeQuote {
    pub fee_amount: String,
    pub fee_denom: String,
    pub expiration: String,
    pub fee_payment_address: String,
}

pub async fn query_skip_api(
    denom: &str,
    amount: String,
    config: &Config,
) -> Result<SkipRouteResponse, AppError> {
    log::info!("Querying Skip API...");
    let client = reqwest::Client::new();
    let request_body = SkipRouteRequest {
        source_asset_denom: denom.to_string(),
        source_asset_chain_id: config.source_asset_chain_id.clone(),
        dest_asset_denom: config.dest_asset_denom.clone(),
        dest_asset_chain_id: config.dest_asset_chain_id.clone(),
        amount_in: amount,
        allow_multi_tx: config.allow_multi_tx,
        allow_unsafe: config.allow_unsafe,
        go_fast: config.go_fast,
        smart_relay: config.smart_relay,
        experimental_features: config.experimental_features.clone(),
        smart_swap_options: SmartSwapOptions {
            split_routes: config.split_routes,
            evm_swaps: config.evm_swaps,
        },
    };

    let res = client
        .post(&config.skip_api_url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    // Check the status code manually
    let status = res.status();

    if status.is_success() {
        // If the code reaches here, the status was a success.
        // We can now safely consume the response to get the JSON body.
        let route_response: SkipRouteResponse = res.json().await?;
        log::debug!("Skip API Response: {route_response:?}");
        Ok(route_response)
    } else {
        // If the status is not success, try to parse the error body
        let error_text = res.text().await?;
        log::error!("Skip API returned an error status ({status}): {error_text}");
        Err(AppError::SkipError {
            error_message: error_text,
        })
    }
}
