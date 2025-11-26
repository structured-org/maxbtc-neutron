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
WBTC_DENOM="ibc/0E293A7622DC9A6439DB60E6D234B5AF446962E27CA3AB44D0590603DFF6968E"
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

    # Current UTC timestamp for salt
    SALT=$(date +%s)

    # 0. Allow List Contract
    echo "--- [0/6] Uploading Allow List Contract ---"
    ALLOW_LIST_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_allow_list.wasm")
    echo ""

    # 1. Exchange Rate Provider Contract
    echo "--- [1/6] Uploading Exchange Rate Provider Contract ---"
    EXCHANGE_RATE_PROVIDER_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_exchange_rate_provider.wasm")
    echo ""

    # 2. Fee Collector Contract (Upload only)
    echo "--- [2/6] Uploading Fee Collector Contract ---"
    FEE_COLLECTOR_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_fee_collector.wasm")
    echo ""

    # 3. Token Contract
    echo "--- [3/6] Uploading Token Contract ---"
    TOKEN_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_token.wasm")
    echo ""

    # 4. Core Contract
    echo "--- [4/6] Uploading Core Contract ---"
    CORE_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_core.wasm")
    echo ""

    # 5. Waitosaur Observer Contract
    echo "--- [5/6] Uploading Waitosaur Observer Contract ---"
    WAITOSAUR_OBSERVER_CODE_ID=$(upload_contract "./integration_tests/artifacts/contracts_thirdparty/waitasaurus.wasm")
    echo ""

    # 6. Waitosaur Holder Contract
    echo "--- [6/6] Uploading Waitosaur Holder Contract ---"
    WAITOSAUR_HOLDER_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_waitosaur_holder.wasm")
    echo ""

    # 7. Withdrawal Manager Contract
    echo "--- [7/6] Uploading Withdrawal Manager Contract ---"
    WITHDRAWAL_MANAGER_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_withdrawal_manager.wasm")
    echo ""

    # 8. Factory Contract
    echo "--- [8/6] Uploading Factory Contract ---"
    FACTORY_CODE_ID=$(upload_contract "$ARTIFACTS_DIR/maxbtc_neutron_factory.wasm")

    FACTORY_INIT_MSG=$(jq -c . <<EOF
{
  "code_ids": {
    "token_code_id": $TOKEN_CODE_ID,
    "core_code_id": $CORE_CODE_ID,
    "exchange_rate_provider_contract_code_id": $EXCHANGE_RATE_PROVIDER_CODE_ID,
    "allowlist_contract_code_id": $ALLOW_LIST_CODE_ID,
    "fee_collector_contract_code_id": $FEE_COLLECTOR_CODE_ID,
    "waitosaur_observer_contract_code_id": $WAITOSAUR_OBSERVER_CODE_ID,
    "waitosaur_holder_contract_code_id": $WAITOSAUR_HOLDER_CODE_ID,
    "withdrawal_manager_contract_code_id": $WITHDRAWAL_MANAGER_CODE_ID
  },
  "salt": "$SALT",
  "owner": "$SENDER_ADDRESS",
  "operator": "$OPERATOR_ADDRESS",
  "ceffu_backend": "$CEFFU_BACKEND_ADDRESS",
  "deposit_denom": "$WBTC_DENOM",
  "deposit_decimals": 6,
  "deposit_cost": "0.01",
  "deposits_cap": null,
  "maxbtc_denom": "maxbtc",
  "fee_collector_params": {
    "collection_period_seconds": 1,
    "fee_apy_reduction_percentage": "0.1"
  },
  "binance_aum_contract": "$BINANCE_AUM_CONTRACT_ADDRESS",
  "waitosaur_observer_unlocker": "$CEFFU_BACKEND_ADDRESS",
  "deposit_forwarder_contract": "$FORWARDER_CONTRACT_ADDRESS"
}
EOF
)
    FACTORY_CONTRACT_ADDRESS=$(instantiate_contract "$FACTORY_CODE_ID" "$FACTORY_INIT_MSG" "maxbtc-neutron-factory")
    echo ""
    echo "Factory Contract Address: $FACTORY_CONTRACT_ADDRESS"

    # Query for the dynamically created Fee Collector Address
    FACTORY_STATE=$(neutrond query wasm contract-state smart "$FACTORY_CONTRACT_ADDRESS" '{"state":{}}' -o json --node "$NODE" | jq -r '.data')
    FEE_COLLECTOR_CONTRACT_ADDRESS=$(echo "$FACTORY_STATE" | jq -r '.fee_collector_contract')
    EXCHANGE_RATE_PROVIDER_CONTRACT_ADDRESS=$(echo "$FACTORY_STATE" | jq -r '.exchange_rate_provider_contract')
    ALLOW_LIST_CONTRACT_ADDRESS=$(echo "$FACTORY_STATE" | jq -r '.allowlist_contract')
    WAITOSAUR_HOLDER_CONTRACT_ADDRESS=$(echo "$FACTORY_STATE" | jq -r '.waitosaur_holder_contract')
    WAITOSAUR_OBSERVER_CONTRACT_ADDRESS=$(echo "$FACTORY_STATE" | jq -r '.waitosaur_observer_contract')
    WITHDRAWAL_MANAGER_CONTRACT_ADDRESS=$(echo "$FACTORY_STATE" | jq -r '.withdrawal_manager_contract')
    CORE_CONTRACT_ADDRESS=$(echo "$FACTORY_STATE" | jq -r '.core_contract')
    

    # Final Output and saving state
    {
        echo "export NODE=$NODE"
        echo "export CHAIN_ID=$CHAIN_ID"
        echo "export OPERATOR_ADDRESS=$OPERATOR_ADDRESS"
        echo "export CEFFU_BACKEND_ADDRESS=$CEFFU_BACKEND_ADDRESS"
        echo "export BINANCE_AUM_CONTRACT_ADDRESS=$BINANCE_AUM_CONTRACT_ADDRESS"
        echo "export FORWARDER_CONTRACT_ADDRESS=$FORWARDER_CONTRACT_ADDRESS"
        echo "export FEE_COLLECTOR_CODE_ID=$FEE_COLLECTOR_CODE_ID"
        echo "export ALLOW_LIST_CODE_ID=$ALLOW_LIST_CODE_ID"
        echo "export EXCHANGE_RATE_PROVIDER_CODE_ID=$EXCHANGE_RATE_PROVIDER_CODE_ID"
        echo "export CORE_CODE_ID=$CORE_CODE_ID"
        echo "export WAITOSAUR_HOLDER_CODE_ID=$WAITOSAUR_HOLDER_CODE_ID"
        echo "export WAITOSAUR_OBSERVER_CODE_ID=$WAITOSAUR_OBSERVER_CODE_ID"
        echo "export WITHDRAWAL_MANAGER_CODE_ID=$WITHDRAWAL_MANAGER_CODE_ID"
        echo "export FORWARDER_CONTRACT_ADDRESS=$FORWARDER_CONTRACT_ADDRESS"
        echo "export CORE_CONTRACT_ADDRESS=$CORE_CONTRACT_ADDRESS"
        echo "export FEE_COLLECTOR_CONTRACT_ADDRESS=$FEE_COLLECTOR_CONTRACT_ADDRESS"
        echo "export EXCHANGE_RATE_PROVIDER_CONTRACT_ADDRESS=$EXCHANGE_RATE_PROVIDER_CONTRACT_ADDRESS"
        echo "export ALLOW_LIST_CONTRACT_ADDRESS=$ALLOW_LIST_CONTRACT_ADDRESS"
        echo "export WAITOSAUR_HOLDER_CONTRACT_ADDRESS=$WAITOSAUR_HOLDER_CONTRACT_ADDRESS"
        echo "export WAITOSAUR_OBSERVER_CONTRACT_ADDRESS=$WAITOSAUR_OBSERVER_CONTRACT_ADDRESS"
        echo "export WITHDRAWAL_MANAGER_CONTRACT_ADDRESS=$WITHDRAWAL_MANAGER_CONTRACT_ADDRESS"
        echo "export FACTORY_CONTRACT_ADDRESS=$FACTORY_CONTRACT_ADDRESS"
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

handle_forwarder_library_command() {
    [[ -z "$FORWARDER_LIBRARY_CONTRACT_ADDRESS" ]] && { echo "Forwarder Library contract address not found. Please run 'setup' first." >&2; exit 1; }

    local sub_command=$1
    shift
    local msg

    case "$sub_command" in
        eureka-transfer)
            local fee_amount=${1:?ERROR: Amount is required}
            local fee_denom=${2:?ERROR: Denom is required}
            local fee_receiver=${3:?ERROR: Receiver address is required}
            local timeout_timestamp=${4:?ERROR: Timeout timestamp is required}
            msg=$(printf '{"process_function":{"eureka_transfer":{"eureka_fee":{"coin":{"amount":"%s","denom":"%s"},"receiver":"%s","timeout_timestamp":%s}}}}' \
                "$fee_amount" "$fee_denom" "$fee_receiver" "$timeout_timestamp")
            echo $msg

            execute_and_wait "Forwarder Library: Eureka Transfer" "$FORWARDER_LIBRARY_CONTRACT_ADDRESS" "$msg"
            ;;
        *)
            echo "Unknown forwarder-library command: $sub_command" >&2
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
    echo "  forwarder-library <sub_command> [<args>]       Execute a message on the Forwarder Library contract."
    echo "  fee-collector <sub_command> [<args>]      Execute a message on the Fee Collector contract."
    echo ""
    echo "Core Sub-commands:"
    echo "  deposit [recipient_addr] <amount><denom>  Deposit funds. e.g., '1000000untrn'"
    echo "  flush-deposits                            Flush pending deposits."
    echo "  update-config '<json_payload>'            Update protocol config (owner only)."
    echo ""
    echo "Forwarder Library Sub-commands:"
    echo "  eureka-transfer <fee_amount> <fee_denom> <fee_receiver_addr> <timeout_nano>"
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
        forwarder-library)
            handle_forwarder_library_command "$@"
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