#!/bin/bash

# -----------------------------------------------------------------------------
# Configurable variables
# -----------------------------------------------------------------------------
NODE="https://rpc-lb.neutron.org"
CHAIN_ID="neutron-1"
GAS="3000000"
FEES="100000untrn"
FROM="populator"
DENOM="ibc/0E293A7622DC9A6439DB60E6D234B5AF446962E27CA3AB44D0590603DFF6968E"
MAXBTC_DENOM="maxbtc"
TREASURY_ADDRESS="neutron12nwelqw8vctn9rlktx6s4lq094e77htyf36ckc"
SENDER_ADDRESS=$(neutrond keys show $FROM -a)
ARTIFACTS_DIR="./artifacts"

# -----------------------------------------------------------------------------
# Helper functions
# -----------------------------------------------------------------------------

# Uploads a contract and returns the code ID.
# All progress is printed to stderr. Only the final code_id is printed to stdout.
# @param $1: Path to the wasm file
upload_contract() {
    local wasm_file=$1
    echo "Uploading contract: $wasm_file..." >&2
    local res
    res=$(neutrond tx wasm store "$wasm_file" --from "$FROM" --node "$NODE" --chain-id "$CHAIN_ID" --gas "$GAS" --fees "$FEES" -o json -y)
    local txhash
    txhash=$(echo "$res" | jq -r '.txhash')

    if [[ -z "$txhash" || "$txhash" == "null" ]]; then
        echo "Error: Failed to get transaction hash." >&2
        echo "Response: $res" >&2
        return 1
    fi

    echo "Transaction hash: $txhash" >&2
    echo -n "Waiting for transaction to be included in a block..." >&2

    local code_id=""
    local attempts=0
    local max_attempts=30 # Timeout after 60 seconds (30 attempts * 2s)

    while [[ -z "$code_id" ]]; do
        sleep 2
        attempts=$((attempts + 1))
        if [[ $attempts -gt $max_attempts ]]; then
            echo "Error: Timeout waiting for transaction confirmation." >&2
            return 1
        fi

        local tx_res
        tx_res=$(neutrond q tx "$txhash" --node "$NODE" -o json 2>/dev/null)
        if [[ $? -ne 0 ]]; then
            echo -n "." >&2 # Still waiting for tx to be indexed
            continue
        fi
        
        local tx_code
        tx_code=$(echo "$tx_res" | jq -r '.code')
        if [[ "$tx_code" -ne 0 ]]; then
            echo >&2 # Newline for readability
            echo "Error: Transaction failed with code $tx_code." >&2
            echo "Raw log: $(echo "$tx_res" | jq -r '.raw_log')" >&2
            return 1
        fi

        code_id=$(echo "$tx_res" | jq -r '.events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value')
        if [[ -n "$code_id" ]]; then
            echo >&2 # Newline for readability
            echo "Success! Found Code ID: $code_id" >&2
            echo "$code_id" # This is the function's return value to stdout
            return 0
        fi
    done
}

# Instantiates a contract and returns the contract address.
# All progress is printed to stderr. Only the final address is printed to stdout.
# @param $1: Code ID
# @param $2: Init message
# @param $3: Label
instantiate_contract() {
    local code_id=$1
    local init_msg=$2
    local label=$3
    echo "Instantiating contract from Code ID $code_id with label '$label'..." >&2
    local res
    res=$(neutrond tx wasm instantiate "$code_id" "$init_msg" --from "$FROM" --admin "$SENDER_ADDRESS" --label "$label" --node "$NODE" --chain-id "$CHAIN_ID" --gas "$GAS" --fees "$FEES" -y -o json)
    local txhash
    txhash=$(echo "$res" | jq -r '.txhash')

    if [[ -z "$txhash" || "$txhash" == "null" ]]; then
        echo "Error: Failed to get transaction hash for instantiation." >&2
        echo "Response: $res" >&2
        return 1
    fi

    echo "Instantiation transaction hash: $txhash" >&2
    echo -n "Waiting for transaction to be included in a block..." >&2

    local contract_address=""
    local attempts=0
    local max_attempts=30 # Timeout after 60 seconds (30 attempts * 2s)

    while [[ -z "$contract_address" ]]; do
        sleep 2
        attempts=$((attempts + 1))
        if [[ $attempts -gt $max_attempts ]]; then
            echo "Error: Timeout waiting for instantiation transaction confirmation." >&2
            return 1
        fi

        local tx_res
        tx_res=$(neutrond q tx "$txhash" --node "$NODE" -o json 2>/dev/null)
        if [[ $? -ne 0 ]]; then
            echo -n "." >&2 # Still waiting for tx to be indexed
            continue
        fi

        local tx_code
        tx_code=$(echo "$tx_res" | jq -r '.code')
        if [[ "$tx_code" -ne 0 ]]; then
            echo >&2 # Newline for readability
            echo "Error: Instantiation transaction failed with code $tx_code." >&2
            echo "Raw log: $(echo "$tx_res" | jq -r '.raw_log')" >&2
            return 1
        fi

        contract_address=$(echo "$tx_res" | jq -r '.events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value')
        if [[ -n "$contract_address" ]]; then
            echo >&2 # Newline for readability
            echo "Success! Found Contract Address: $contract_address" >&2
            echo "$contract_address" # This is the function's return value to stdout
            return 0
        fi
    done
}

# -----------------------------------------------------------------------------
# Contract instantiation
# -----------------------------------------------------------------------------

# Collector
COLLECTOR_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_collector.wasm")
[[ $? -ne 0 ]] && exit 1
COLLECTOR_CONTRACT_ADDRESS=$(instantiate_contract "$COLLECTOR_CODE_ID" '{}' "maxbtc-collector")
[[ $? -ne 0 ]] && exit 1

# AUM Oracle
AUM_ORACLE_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_aum_oracle.wasm")
[[ $? -ne 0 ]] && exit 1
AUM_ORACLE_CONTRACT_ADDRESS=$(instantiate_contract "$AUM_ORACLE_CODE_ID" '{"aum": "0"}' "maxbtc-aum-oracle")
[[ $? -ne 0 ]] && exit 1

# Liquidation Buffer
LIQUIDATION_BUFFER_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_liquidation_buffer.wasm")
[[ $? -ne 0 ]] && exit 1
LIQUIDATION_BUFFER_CONTRACT_ADDRESS=$(instantiate_contract "$LIQUIDATION_BUFFER_CODE_ID" '{"owned_maxbtc": "0", "owned_btc": "0"}' "maxbtc-liquidation-buffer")
[[ $? -ne 0 ]] && exit 1

# Pump
PUMP_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_pump.wasm")
[[ $? -ne 0 ]] && exit 1
PUMP_CONTRACT_ADDRESS=$(instantiate_contract "$PUMP_CODE_ID" '{}' "maxbtc-pump")
[[ $? -ne 0 ]] && exit 1

# Core
CORE_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_core.wasm")
[[ $? -ne 0 ]] && exit 1
CORE_INIT_MSG=$(cat <<EOF
{
    "aum_contract": "$AUM_ORACLE_CONTRACT_ADDRESS",
    "collector_contract": "$COLLECTOR_CONTRACT_ADDRESS",
    "treasury_address": "$TREASURY_ADDRESS",
    "liquidation_buffer_contract": "$LIQUIDATION_BUFFER_CONTRACT_ADDRESS",
    "deposit_pump_contract": "$PUMP_CONTRACT_ADDRESS",
    "accepted_withdrawable_percentage": "0.005",
    "batch_active_duration": 10,
    "batch_withdrawing_duration": 10,
    "cached_aum_tolerance": "0.02",
    "cached_er_ttl": 20,
    "deposit_decimals": 6,
    "deposit_denom": "$DENOM",
    "deposit_cost": "0.01",
    "deposit_flush_period": 10,
    "liquidation_buffer_share": "0.1",
    "maxbtc_denom": "$MAXBTC_DENOM",
    "owner": "$SENDER_ADDRESS"
}
EOF
)
CORE_CONTRACT_ADDRESS=$(instantiate_contract "$CORE_CODE_ID" "$CORE_INIT_MSG" "maxbtc-core")
[[ $? -ne 0 ]] && exit 1

# -----------------------------------------------------------------------------
# Output
# -----------------------------------------------------------------------------

echo "------------------------------------------------"
echo "Collector Contract Address: $COLLECTOR_CONTRACT_ADDRESS"
echo "AUM Oracle Contract Address: $AUM_ORACLE_CONTRACT_ADDRESS"
echo "Liquidation Buffer Contract Address: $LIQUIDATION_BUFFER_CONTRACT_ADDRESS"
echo "Pump Contract Address: $PUMP_CONTRACT_ADDRESS"
echo "Core Contract Address: $CORE_CONTRACT_ADDRESS"
echo "------------------------------------------------"