mod chain;
mod config;
mod error;
mod skip_api;

use crate::chain::{ChainClient, EurekaFee, ExecuteMsg};
use crate::config::Config;
use crate::error::AppError;
use crate::skip_api::query_skip_api;
use chrono::DateTime;
use cosmwasm_std::{Coin, Uint128};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    dotenv::dotenv().ok();
    env_logger::init();

    let config = Config::from_env()?;
    log::info!("Configuration loaded successfully.");

    let chain_client = ChainClient::new(&config).await?;
    log::info!("Neutron client initialized.");

    loop {
        log::info!("Starting new execution cycle...");
        match run_cycle(&config, &chain_client).await {
            Ok(_) => log::info!("Execution cycle completed successfully."),
            Err(e) => log::error!("Execution cycle failed: {}", e),
        }
        log::info!("Sleeping for {} seconds.", config.execute_period);
        tokio::time::sleep(Duration::from_secs(config.execute_period)).await;
    }
}

async fn run_cycle(config: &Config, chain_client: &ChainClient) -> Result<(), AppError> {
    let mut balance = chain_client
        .query_balance(&config.contract_address, &config.neutron_denom)
        .await?;
    log::info!(
        "Queried balance for contract {}: {} {}",
        config.contract_address,
        balance,
        config.neutron_denom
    );

    // if balance == 0 {
    //     log::info!("Balance is zero, skipping transaction.");
    //     return Ok(());
    // }

    balance = 100000;

    let skip_response = query_skip_api(&config.neutron_denom, balance.to_string(), config).await?;

    let eureka_transfer = skip_response
        .operations
        .into_iter()
        .find_map(|op| op.eureka_transfer)
        .ok_or_else(|| {
            AppError::Chain("No eureka_transfer operation found in Skip API response".to_string())
        })?;

    log::debug!("Skip API eureka transfer info: {:?}", eureka_transfer);

    let fee_quote = eureka_transfer.smart_relay_fee_quote;
    if fee_quote.fee_denom != config.hub_denom {
        return Err(AppError::DenomMismatch {
            expected: config.neutron_denom.clone(),
            got: fee_quote.fee_denom,
        });
    }

    let fee_amount = fee_quote
        .fee_amount
        .parse::<u128>()
        .map_err(|_| AppError::Chain("Failed to parse fee amount from Skip API".to_string()))?;

    // Parse the expiration timestamp and convert to nanoseconds
    let timeout_timestamp_nano = DateTime::parse_from_rfc3339(&fee_quote.expiration)
        .map_err(|e| AppError::Chain(format!("Failed to parse expiration timestamp: {}", e)))?
        .timestamp_nanos_opt()
        .unwrap() as u64;

    let eureka_fee = EurekaFee {
        coin: Coin {
            denom: fee_quote.fee_denom,
            amount: Uint128::from(fee_amount),
        },
        receiver: fee_quote.fee_payment_address,
        timeout_timestamp: timeout_timestamp_nano,
    };

    let msg = ExecuteMsg::EurekaTransfer { eureka_fee };
    log::debug!("EurekaTransfer message: {:?}", msg);

    let tx_hash = chain_client
        .execute_message(&config.contract_address, msg)
        .await?;
    log::info!("Transfer tx hash: {}", tx_hash);
    Ok(())
}
