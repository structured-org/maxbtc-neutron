#!/bin/bash

# Exit script on any error
set -e

# -----------------------------------------------------------------------------
# Configurable variables
# -----------------------------------------------------------------------------
NODE="https://rpc-lb.neutron.org"
CHAIN_ID="neutron-1"
FEES="100000untrn"
FROM="populator"
GAS="3200000"
DENOM="ibc/0E293A7622DC9A6439DB60E6D234B5AF446962E27CA3AB44D0590603DFF6968E"
MAXBTC_DENOM="maxbtc"
TREASURY_ADDRESS="neutron12nwelqw8vctn9rlktx6s4lq094e77htyf36ckc"
ARTIFACTS_DIR="./artifacts"
SENDER_ADDRESS=$(neutrond keys show $FROM -a)
DEPLOYMENT_ENV_FILE="deployment.env"

# Load deployment variables if they exist
if [ -f "$DEPLOYMENT_ENV_FILE" ]; then
    source "$DEPLOYMENT_ENV_FILE"
fi

# -----------------------------------------------------------------------------
# Helper functions
# -----------------------------------------------------------------------------

# Uploads a contract and returns the code ID.
upload_contract() {
    local wasm_file=$1
    echo "Uploading contract: $wasm_file..." >&2
    local res
    res=$(neutrond tx wasm store "$wasm_file" --from "$FROM" --node "$NODE" --chain-id "$CHAIN_ID" --gas-adjustment 1.5 --gas "$GAS" --fees "$FEES" -o json -y)
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
    for ((attempts=0; attempts<30; attempts++)); do
        sleep 2
        local tx_res
        tx_res=$(neutrond q tx "$txhash" --node "$NODE" -o json 2>/dev/null || continue)
        
        local tx_code
        tx_code=$(echo "$tx_res" | jq -r '.code')
        if [[ "$tx_code" -ne 0 ]]; then
            echo -e "\nError: Transaction failed with code $tx_code." >&2
            echo "Raw log: $(echo "$tx_res" | jq -r '.raw_log')" >&2
            return 1
        fi

        code_id=$(echo "$tx_res" | jq -r '.events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value')
        if [[ -n "$code_id" ]]; then
            echo -e "\nSuccess! Found Code ID: $code_id" >&2
            echo "$code_id"
            return 0
        fi
        echo -n "." >&2
    done

    echo "Error: Timeout waiting for transaction confirmation." >&2
    return 1
}

# Instantiates a contract and returns the contract address.
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
    for ((attempts=0; attempts<30; attempts++)); do
        sleep 2
        local tx_res
        tx_res=$(neutrond q tx "$txhash" --node "$NODE" -o json 2>/dev/null || continue)

        local tx_code
        tx_code=$(echo "$tx_res" | jq -r '.code')
        if [[ "$tx_code" -ne 0 ]]; then
            echo -e "\nError: Instantiation transaction failed with code $tx_code." >&2
            echo "Raw log: $(echo "$tx_res" | jq -r '.raw_log')" >&2
            return 1
        fi

        contract_address=$(echo "$tx_res" | jq -r '.events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value' | head -n 1)
        if [[ -n "$contract_address" ]]; then
            echo -e "\nSuccess! Found Contract Address: $contract_address" >&2
            echo "$contract_address"
            return 0
        fi
        echo -n "." >&2
    done

    echo "Error: Timeout waiting for instantiation transaction confirmation." >&2
    return 1
}

# Executes a contract message and waits for confirmation.
execute_and_wait() {
    local description=$1
    local contract_address=$2
    local msg=$3
    local funds=$4 # Optional: --amount flag

    echo "Executing: $description..." >&2
    
    local cmd_args=()
    if [[ -n "$funds" ]]; then
        cmd_args+=(--amount "$funds")
    fi
    
    local res
    res=$(neutrond tx wasm execute "$contract_address" "$msg" --from "$FROM" "${cmd_args[@]}" --node "$NODE" --chain-id "$CHAIN_ID" --gas-adjustment 1.5 --gas "$GAS" --fees "$FEES" -y -o json)
    
    local txhash
    txhash=$(echo "$res" | jq -r '.txhash')

    if [[ -z "$txhash" || "$txhash" == "null" ]]; then
        echo "Error: Failed to get transaction hash for execution." >&2
        echo "Response: $res" >&2
        return 1
    fi

    echo "Execution transaction hash: $txhash" >&2
    echo -n "Waiting for transaction to be included in a block..." >&2

    for ((attempts=0; attempts<30; attempts++)); do
        sleep 2
        local tx_res
        tx_res=$(neutrond q tx "$txhash" --node "$NODE" -o json 2>/dev/null || continue)
        
        local tx_code
        tx_code=$(echo "$tx_res" | jq -r '.code' 2>/dev/null)
        
        if [[ "$tx_code" == "0" ]]; then
            echo -e "\n✅ Success: Transaction confirmed." >&2
            return 0
        elif [[ -n "$tx_code" ]]; then
            echo -e "\n❌ Error: Execution transaction failed with code $tx_code." >&2
            echo "Raw log: $(echo "$tx_res" | jq -r '.raw_log')" >&2
            return 1
        fi
        echo -n "." >&2
    done

    echo "Error: Timeout waiting for execution transaction confirmation." >&2
    return 1
}


# -----------------------------------------------------------------------------
# Command-specific functions
# -----------------------------------------------------------------------------

run_setup() {
    echo "Starting contract deployment..."
    echo "Sender Address: $SENDER_ADDRESS"
    echo ""

    # 1. Collector Contract
    echo "--- [1/8] Deploying Collector Contract ---"
    COLLECTOR_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_collector.wasm")
    COLLECTOR_CONTRACT_ADDRESS=$(instantiate_contract "$COLLECTOR_CODE_ID" '{}' "maxbtc-collector")
    echo ""

    # 2. AUM Oracle Contract
    echo "--- [2/8] Deploying AUM Oracle Contract ---"
    AUM_ORACLE_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_aum_oracle.wasm")
    AUM_ORACLE_CONTRACT_ADDRESS=$(instantiate_contract "$AUM_ORACLE_CODE_ID" '{"aum": "0"}' "maxbtc-aum-oracle")
    echo ""

    # 3. Liquidation Buffer Contract
    echo "--- [3/8] Deploying Liquidation Buffer Contract ---"
    LIQUIDATION_BUFFER_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_liquidation_buffer.wasm")
    LIQUIDATION_BUFFER_CONTRACT_ADDRESS=$(instantiate_contract "$LIQUIDATION_BUFFER_CODE_ID" '{"owned_maxbtc": "0", "owned_btc": "0"}' "maxbtc-liquidation-buffer")
    echo ""

    # 4. Fee Collector Contract (Upload only)
    echo "--- [4/8] Uploading Fee Collector Contract ---"
    FEE_COLLECTOR_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_fee_collector.wasm")
    echo ""

    # 5. Pump Contract (Valence Base Account)
    echo "--- [5/8] Deploying Pump Contract (Valence Base Account) ---"
    PUMP_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/valence_base_account.wasm")
    PUMP_INIT_MSG=$(printf '{"admin": "%s", "approved_libraries": []}' "$SENDER_ADDRESS")
    PUMP_CONTRACT_ADDRESS=$(instantiate_contract "$PUMP_CODE_ID" "$PUMP_INIT_MSG" "valence-pump")
    echo ""

    # 6. Pump Library Contract (Valence IBC Transfer Library)
    echo "--- [6/8] Deploying Pump Library Contract ---"
    PUMP_LIBRARY_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/valence_neutron_ibc_transfer_library.wasm")
    PUMP_LIBRARY_INIT_MSG=$(jq -c . <<EOF
{
  "owner": "$SENDER_ADDRESS",
  "processor": "$SENDER_ADDRESS",
  "config": {
    "input_addr": { "library_account_addr": "$PUMP_CONTRACT_ADDRESS" },
    "output_addr": { "library_account_addr": "0x1234567890123456789012345678901234567890" },
    "denom": { "native": "$DENOM" },
    "amount": "full_amount",
    "memo": "",
    "remote_chain_info": { "channel_id": "channel-1" },
    "denom_to_pfm_map": {},
    "eureka_config": {
      "callback_contract": "cosmos1lqu9662kd4my6dww4gzp3730vew0gkwe0nl9ztjh0n5da0a8zc4swsvd22",
      "action_contract": "cosmos1clswlqlfm8gpn7n5wu0ypu0ugaj36urlhj7yz30hn7v7mkcm2tuqy9f8s5",
      "recover_address": "cosmos1ep2umj6kn34g2ttjalsc5r9w8pt7sv4x9z0q26",
      "source_channel": "08-wasm-1369"
    }
  }
}
EOF
)
    PUMP_LIBRARY_CONTRACT_ADDRESS=$(instantiate_contract "$PUMP_LIBRARY_CODE_ID" "$PUMP_LIBRARY_INIT_MSG" "valence-pump-library")
    echo ""

    # 7. Approve Pump Library
    echo "--- [7/8] Approving Pump Library ---"
    APPROVE_MSG=$(printf '{"approve_library":{"library":"%s"}}' "$PUMP_LIBRARY_CONTRACT_ADDRESS")
    execute_and_wait "Approve Pump Library" "$PUMP_CONTRACT_ADDRESS" "$APPROVE_MSG"
    echo ""

    # 8. Core Contract
    echo "--- [8/8] Deploying Core Contract ---"
    CORE_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_core.wasm")
    CORE_INIT_MSG=$(jq -c . <<EOF
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
  "deposit_denom": "untrn",
  "deposit_cost": "0.01",
  "deposit_flush_period": 10,
  "liquidation_buffer_share": "0.1",
  "maxbtc_denom": "$MAXBTC_DENOM",
  "owner": "$SENDER_ADDRESS",
  "fee_collector_params": {
    "code_id": $FEE_COLLECTOR_CODE_ID,
    "collection_period_seconds": 1,
    "fee_apy_reduction_percentage": "0.1",
    "salt": "ZmVlX2NvbGxlY3Rvcl9zYWx0"
  }
}
EOF
)
    CORE_CONTRACT_ADDRESS=$(instantiate_contract "$CORE_CODE_ID" "$CORE_INIT_MSG" "maxbtc-core")
    echo ""

    # Query for the dynamically created Fee Collector Address
    FEE_COLLECTOR_CONTRACT_ADDRESS=$(neutrond query wasm contract-state smart "$CORE_CONTRACT_ADDRESS" '{"config":{}}' -o json --node "$NODE" | jq -r '.data.fee_collector_contract')
    if [[ -z "$FEE_COLLECTOR_CONTRACT_ADDRESS" || "$FEE_COLLECTOR_CONTRACT_ADDRESS" == "null" ]]; then
        echo "Warning: Could not query Fee Collector address from Core contract." >&2
    else
        echo "Queried Fee Collector Address: $FEE_COLLECTOR_CONTRACT_ADDRESS"
    fi

    # Final Output and saving state
    {
        echo "export COLLECTOR_CODE_ID=$COLLECTOR_CODE_ID"
        echo "export AUM_ORACLE_CODE_ID=$AUM_ORACLE_CODE_ID"
        echo "export LIQUIDATION_BUFFER_CODE_ID=$LIQUIDATION_BUFFER_CODE_ID"
        echo "export FEE_COLLECTOR_CODE_ID=$FEE_COLLECTOR_CODE_ID"
        echo "export PUMP_CODE_ID=$PUMP_CODE_ID"
        echo "export PUMP_LIBRARY_CODE_ID=$PUMP_LIBRARY_CODE_ID"
        echo "export CORE_CODE_ID=$CORE_CODE_ID"
        echo "export COLLECTOR_CONTRACT_ADDRESS=$COLLECTOR_CONTRACT_ADDRESS"
        echo "export AUM_ORACLE_CONTRACT_ADDRESS=$AUM_ORACLE_CONTRACT_ADDRESS"
        echo "export LIQUIDATION_BUFFER_CONTRACT_ADDRESS=$LIQUIDATION_BUFFER_CONTRACT_ADDRESS"
        echo "export PUMP_CONTRACT_ADDRESS=$PUMP_CONTRACT_ADDRESS"
        echo "export PUMP_LIBRARY_CONTRACT_ADDRESS=$PUMP_LIBRARY_CONTRACT_ADDRESS"
        echo "export CORE_CONTRACT_ADDRESS=$CORE_CONTRACT_ADDRESS"
        echo "export FEE_COLLECTOR_CONTRACT_ADDRESS=$FEE_COLLECTOR_CONTRACT_ADDRESS"
    } > "$DEPLOYMENT_ENV_FILE"

    echo "================================================================="
    echo "✅ Deployment Complete. Configuration saved to $DEPLOYMENT_ENV_FILE"
    echo "================================================================="
    cat "$DEPLOYMENT_ENV_FILE"
    echo "================================================================="
}

handle_core_command() {
    [[ -z "$CORE_CONTRACT_ADDRESS" ]] && { echo "Core contract address not found. Please run 'setup' first." >&2; exit 1; }
    
    local sub_command=$1
    shift
    local msg

    case "$sub_command" in
        deposit)
            local recipient=${1:-$SENDER_ADDRESS}
            local amount=${2:?ERROR: Amount to deposit is required (e.g., 1000untrn)}
            msg=$(printf '{"deposit":{"recipient":"%s"}}' "$recipient")
            execute_and_wait "Core: Deposit" "$CORE_CONTRACT_ADDRESS" "$msg" "$amount"
            ;;
        flush-deposits)
            msg='{"flush_deposits":{}}'
            execute_and_wait "Core: Flush Deposits" "$CORE_CONTRACT_ADDRESS" "$msg"
            ;;
        withdraw)
            local amount=${1:?ERROR: Amount of maxBTC to withdraw is required (e.g., 1000umaxbtc)}
            msg='{"withdraw":{}}'
            execute_and_wait "Core: Withdraw" "$CORE_CONTRACT_ADDRESS" "$msg" "$amount"
            ;;
        process-active-batch)
            msg='{"process_active_batch":{}}'
            execute_and_wait "Core: Process Active Batch" "$CORE_CONTRACT_ADDRESS" "$msg"
            ;;
        claim)
            local recipient=${1:-$SENDER_ADDRESS}
            msg=$(printf '{"claim":{"recipient":"%s"}}' "$recipient")
            execute_and_wait "Core: Claim" "$CORE_CONTRACT_ADDRESS" "$msg"
            ;;
        process-cache)
            msg='{"process_cache":{}}'
            execute_and_wait "Core: Process Cache" "$CORE_CONTRACT_ADDRESS" "$msg"
            ;;
        update-config)
            local config_json=${1:?ERROR: JSON object for config update is required}
            msg=$(printf '{"update_config":%s}' "$config_json")
            execute_and_wait "Core: Update Config" "$CORE_CONTRACT_ADDRESS" "$msg"
            ;;
        *)
            echo "Unknown core command: $sub_command" >&2
            usage
            ;;
    esac
}

handle_pump_library_command() {
    [[ -z "$PUMP_LIBRARY_CONTRACT_ADDRESS" ]] && { echo "Pump Library contract address not found. Please run 'setup' first." >&2; exit 1; }

    local sub_command=$1
    shift
    local msg

    case "$sub_command" in
        eureka-transfer)
            local amount=${1:?ERROR: Amount is required}
            local denom=${2:?ERROR: Denom is required}
            local receiver=${3:?ERROR: Receiver address is required}
            # Timeout in seconds from now. Default: 600s (10 min)
            local timeout_seconds=${4:-600}
            local timeout_nanos=$(( ($(date +%s) + timeout_seconds) * 1000000000 ))

            msg=$(printf '{"eureka_transfer":{"eureka_fee":{"coin":{"amount":"%s","denom":"%s"},"receiver":"%s","timeout_timestamp":"%s"}}}' \
                "$amount" "$denom" "$receiver" "$timeout_nanos")
            
            execute_and_wait "Pump Library: Eureka Transfer" "$PUMP_LIBRARY_CONTRACT_ADDRESS" "$msg"
            ;;
        *)
            echo "Unknown pump-library command: $sub_command" >&2
            usage
            ;;
    esac
}

handle_fee_collector_command() {
    [[ -z "$FEE_COLLECTOR_CONTRACT_ADDRESS" ]] && { echo "Fee Collector contract address not found. Please run 'setup' first." >&2; exit 1; }

    local sub_command=$1
    shift
    local msg

    case "$sub_command" in
        claim)
            local amount=${1:?ERROR: Amount is required}
            local denom=${2:?ERROR: Denom is required}
            msg=$(printf '{"claim":{"amount":{"amount":"%s","denom":"%s"}}}' "$amount" "$denom")
            execute_and_wait "Fee Collector: Claim" "$FEE_COLLECTOR_CONTRACT_ADDRESS" "$msg"
            ;;
        *)
            echo "Unknown fee-collector command: $sub_command" >&2
            usage
            ;;
    esac
}

usage() {
    echo "Usage: $0 <command> [<sub_command>] [<args>]"
    echo ""
    echo "Commands:"
    echo "  setup                                     Uploads and instantiates all contracts."
    echo "  core <sub_command> [<args>]               Execute a message on the Core contract."
    echo "  pump-library <sub_command> [<args>]       Execute a message on the Pump Library contract."
    echo "  fee-collector <sub_command> [<args>]      Execute a message on the Fee Collector contract."
    echo ""
    echo "Core Sub-commands:"
    echo "  deposit [recipient_addr] <amount><denom>  Deposit funds. e.g., '1000000untrn'"
    echo "  withdraw <amount><maxbtc_denom>           Withdraw maxBTC. e.g., '500000umaxbtc'"
    echo "  flush-deposits                            Flush pending deposits."
    echo "  process-active-batch                      Process the active withdrawal batch."
    echo "  claim [recipient_addr]                    Claim withdrawn BTC."
    echo "  process-cache                             Trigger cache processing."
    echo "  update-config '<json_payload>'            Update protocol config (owner only)."
    echo ""
    echo "Pump Library Sub-commands:"
    echo "  eureka-transfer <amount> <denom> <receiver_addr> [timeout_sec]"
    echo "                                            Execute a Eureka transfer."
    echo ""
    echo "Fee Collector Sub-commands:"
    echo "  claim <amount> <denom>                    Claim fees from the collector."
    echo ""
}

# -----------------------------------------------------------------------------
# Main script execution
# -----------------------------------------------------------------------------
main() {
    if [[ $# -eq 0 ]]; then
        usage
        exit 1
    fi

    local command=$1
    shift

    case "$command" in
        setup)
            run_setup
            ;;
        core)
            handle_core_command "$@"
            ;;
        pump-library)
            handle_pump_library_command "$@"
            ;;
        fee-collector)
            handle_fee_collector_command "$@"
            ;;
        *)
            echo "Unknown command: $command"
            usage
            exit 1
            ;;
    esac
}

main "$@"