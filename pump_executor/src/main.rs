use valence_domain_clients::clients::{
    neutron::NeutronClient,
};
use valence_domain_clients::cosmos::wasm_client::WasmClient;
use valence_domain_clients::cosmos::base_client::BaseClient;
use cosmwasm_schema::{cw_serde};
use cosmwasm_std::{Coin, Uint128};

#[cw_serde]
pub enum ExecuteMsg {
    Push {
        // How much to transfer.
        amount: Coin,
        // The Eureka fee data that is retrieved from Skip API.
        eureka_fee: EurekaFee,
        // The contract on Cosmos Hub to which we send the original transfer.
        to_chain_entry_contract_address: String,
        // The contract which is called by the entry contract on Cosmos Hub to actually process the
        // transfer.
        to_chain_callback_contract_address: String,
        // The Eureka source channel on Cosmos Hub.
        eureka_source_channel: String,
        // Expected format: 1750089037000000000 (unix nano)
        eureka_full_timeout_nano: u64,
    },
}

#[cw_serde]
pub struct EurekaFee {
    pub coin: Coin,
    pub receiver: String,
    // Expected format: 1750089037000000000 (unix nano)
    pub timeout_timestamp: u64,
}


#[tokio::main]
async fn main() {
    let neutron_client = NeutronClient::new(
        "https://rpc.neutron.quokkastake.io",
        "9090",
        "stamp walnut alarm response kidney possible crack mass rent screen summer drastic junior spatial forest analyst prize insane okay unfair screen liar match blanket",
        "neutron-1",
    )
        .await.unwrap();
    let tx_resp = neutron_client
        .execute_wasm(
            "neutron1w798gp0zqv3s9hjl3jlnwxtwhykga6rn93p46q2crsdqhaj3y4gsum0096",
            ExecuteMsg::Push {
                amount: Coin{amount:Uint128::one(), denom:"some_denom".to_string()},
                eureka_fee: EurekaFee {
                    coin: Default::default(),
                    receiver: "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599".to_string(),
                    timeout_timestamp: 1750089037000000000,
                },
                to_chain_entry_contract_address: "cosmos1clswlqlfm8gpn7n5wu0ypu0ugaj36urlhj7yz30hn7v7mkcm2tuqy9f8s5".to_string(),
                to_chain_callback_contract_address: "cosmos1lqu9662kd4my6dww4gzp3730vew0gkwe0nl9ztjh0n5da0a8zc4swsvd22".to_string(),
                eureka_source_channel: "08-wasm-1369".to_string(),
                eureka_full_timeout_nano: 1750089037000000000,
            },
            vec![],
            None,
        )
        .await.unwrap();


    let tx_result = neutron_client.poll_for_tx(&tx_resp.hash).await.unwrap();
    println!("{:?}", tx_result.code);

    // returns u128
    let balance_128 = neutron_client
        .query_balance(
            "neutron1w798gp0zqv3s9hjl3jlnwxtwhykga6rn93p46q2crsdqhaj3y4gsum0096",
            "some_denom",
        )
        .await.unwrap();
    println!("{:?}", balance_128);
}

