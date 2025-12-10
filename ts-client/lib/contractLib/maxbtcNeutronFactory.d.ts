import { CosmWasmClient, SigningCosmWasmClient, ExecuteResult, InstantiateResult } from "@cosmjs/cosmwasm-stargate";
import { StdFee } from "@cosmjs/amino";
/**
 * Expiration represents a point in time when some event happens. It can compare with a BlockInfo and will return is_expired() == true once the condition is hit (and for every block in the future)
 */
export type Expiration = {
    at_height: number;
} | {
    at_time: Timestamp;
} | {
    never: {};
};
/**
 * A point in time in nanosecond precision.
 *
 * This type can represent times from 1970-01-01T00:00:00Z to 2554-07-21T23:34:33Z.
 *
 * ## Examples
 *
 * ``` # use cosmwasm_std::Timestamp; let ts = Timestamp::from_nanos(1_000_000_202); assert_eq!(ts.nanos(), 1_000_000_202); assert_eq!(ts.seconds(), 1); assert_eq!(ts.subsec_nanos(), 202);
 *
 * let ts = ts.plus_seconds(2); assert_eq!(ts.nanos(), 3_000_000_202); assert_eq!(ts.seconds(), 3); assert_eq!(ts.subsec_nanos(), 202); ```
 */
export type Timestamp = Uint64;
/**
 * A thin wrapper around u64 that is using strings for JSON encoding/decoding, such that the full u64 range can be used for clients that convert JSON numbers to floats, like JavaScript and jq.
 *
 * # Examples
 *
 * Use `from` to create instances of this and `u64` to get the value out:
 *
 * ``` # use cosmwasm_std::Uint64; let a = Uint64::from(42u64); assert_eq!(a.u64(), 42);
 *
 * let b = Uint64::from(70u32); assert_eq!(b.u64(), 70); ```
 */
export type Uint64 = string;
/**
 * A human readable address.
 *
 * In Cosmos, this is typically bech32 encoded. But for multi-chain smart contracts no assumptions should be made other than being UTF-8 encoded and of reasonable length.
 *
 * This type represents a validated address. It can be created in the following ways 1. Use `Addr::unchecked(input)` 2. Use `let checked: Addr = deps.api.addr_validate(input)?` 3. Use `let checked: Addr = deps.api.addr_humanize(canonical_addr)?` 4. Deserialize from JSON. This must only be done from JSON that was validated before such as a contract's state. `Addr` must not be used in messages sent by the user because this would result in unvalidated instances.
 *
 * This type is immutable. If you really need to mutate it (Really? Are you sure?), create a mutable copy using `let mut mutable = Addr::to_string()` and operate on that `String` instance.
 */
export type Addr = string;
export type CosmosMsgFor_Empty = {
    bank: BankMsg;
} | {
    custom: Empty;
} | {
    any: AnyMsg;
} | {
    wasm: WasmMsg;
};
/**
 * The message types of the bank module.
 *
 * See https://github.com/cosmos/cosmos-sdk/blob/v0.40.0/proto/cosmos/bank/v1beta1/tx.proto
 */
export type BankMsg = {
    send: {
        amount: Coin[];
        to_address: string;
    };
} | {
    burn: {
        amount: Coin[];
    };
};
/**
 * A thin wrapper around u128 that is using strings for JSON encoding/decoding, such that the full u128 range can be used for clients that convert JSON numbers to floats, like JavaScript and jq.
 *
 * # Examples
 *
 * Use `from` to create instances of this and `u128` to get the value out:
 *
 * ``` # use cosmwasm_std::Uint128; let a = Uint128::from(123u128); assert_eq!(a.u128(), 123);
 *
 * let b = Uint128::from(42u64); assert_eq!(b.u128(), 42);
 *
 * let c = Uint128::from(70u32); assert_eq!(c.u128(), 70); ```
 */
export type Uint128 = string;
/**
 * Binary is a wrapper around Vec<u8> to add base64 de/serialization with serde. It also adds some helper methods to help encode inline.
 *
 * This is only needed as serde-json-{core,wasm} has a horrible encoding for Vec<u8>. See also <https://github.com/CosmWasm/cosmwasm/blob/main/docs/MESSAGE_TYPES.md>.
 */
export type Binary = string;
/**
 * The message types of the wasm module.
 *
 * See https://github.com/CosmWasm/wasmd/blob/v0.14.0/x/wasm/internal/types/tx.proto
 */
export type WasmMsg = {
    execute: {
        contract_addr: string;
        funds: Coin[];
        /**
         * msg is the json-encoded ExecuteMsg struct (as raw Binary)
         */
        msg: Binary;
    };
} | {
    instantiate: {
        admin?: string | null;
        code_id: number;
        funds: Coin[];
        /**
         * A human-readable label for the contract.
         *
         * Valid values should: - not be empty - not be bigger than 128 bytes (or some chain-specific limit) - not start / end with whitespace
         */
        label: string;
        /**
         * msg is the JSON-encoded InstantiateMsg struct (as raw Binary)
         */
        msg: Binary;
    };
} | {
    instantiate2: {
        admin?: string | null;
        code_id: number;
        funds: Coin[];
        /**
         * A human-readable label for the contract.
         *
         * Valid values should: - not be empty - not be bigger than 128 bytes (or some chain-specific limit) - not start / end with whitespace
         */
        label: string;
        /**
         * msg is the JSON-encoded InstantiateMsg struct (as raw Binary)
         */
        msg: Binary;
        salt: Binary;
    };
} | {
    migrate: {
        contract_addr: string;
        /**
         * msg is the json-encoded MigrateMsg struct that will be passed to the new code
         */
        msg: Binary;
        /**
         * the code_id of the new logic to place in the given contract
         */
        new_code_id: number;
    };
} | {
    update_admin: {
        admin: string;
        contract_addr: string;
    };
} | {
    clear_admin: {
        contract_addr: string;
    };
};
/**
 * Actions that can be taken to alter the contract's ownership
 */
export type UpdateOwnershipArgs = {
    transfer_ownership: {
        expiry?: Expiration | null;
        new_owner: string;
    };
} | "accept_ownership" | "renounce_ownership";
/**
 * A fixed-point decimal value with 18 fractional digits, i.e. Decimal(1_000_000_000_000_000_000) == 1.0
 *
 * The greatest possible value that can be represented is 340282366920938463463.374607431768211455 (which is (2^128 - 1) / 10^18)
 */
export type Decimal = string;
export interface MaxbtcNeutronFactorySchema {
    responses: OwnershipForString | State;
    execute: AdminExecuteArgs | UpdateOwnershipArgs;
    instantiate?: InstantiateMsg;
    [k: string]: unknown;
}
/**
 * The contract's ownership info
 */
export interface OwnershipForString {
    /**
     * The contract's current owner. `None` if the ownership has been renounced.
     */
    owner?: string | null;
    /**
     * The deadline for the pending owner to accept the ownership. `None` if there isn't a pending ownership transfer, or if a transfer exists and it doesn't have a deadline.
     */
    pending_expiry?: Expiration | null;
    /**
     * The account who has been proposed to take over the ownership. `None` if there isn't a pending ownership transfer.
     */
    pending_owner?: string | null;
}
export interface State {
    allowlist_contract: Addr;
    core_contract: Addr;
    exchange_rate_provider_contract: Addr;
    fee_collector_contract: Addr;
    token_contract: Addr;
    waitosaur_holder_contract: Addr;
    waitosaur_observer_contract: Addr;
    withdrawal_manager_contract: Addr;
}
export interface AdminExecuteArgs {
    msgs: CosmosMsgFor_Empty[];
}
export interface Coin {
    amount: Uint128;
    denom: string;
}
/**
 * An empty struct that serves as a placeholder in different places, such as contracts that don't set a custom message.
 *
 * It is designed to be expressible in correct JSON and JSON Schema but contains no meaningful data. Previously we used enums without cases, but those cannot represented as valid JSON Schema (https://github.com/CosmWasm/cosmwasm/issues/451)
 */
export interface Empty {
}
/**
 * A message encoded the same way as a protobuf [Any](https://github.com/protocolbuffers/protobuf/blob/master/src/google/protobuf/any.proto). This is the same structure as messages in `TxBody` from [ADR-020](https://github.com/cosmos/cosmos-sdk/blob/master/docs/architecture/adr-020-protobuf-transaction-encoding.md)
 */
export interface AnyMsg {
    type_url: string;
    value: Binary;
}
/**
 * InstantiateMsg configures the contract on initialization.
 */
export interface InstantiateMsg {
    /**
     * Address of the binance AUM contract
     */
    binance_aum_contract: string;
    ceffu_backend: string;
    code_ids: CodeIds;
    /**
     * One-off cost (Decimal) charged when a user deposits to mint maxBTC
     */
    deposit_cost: Decimal;
    /**
     * Number of decimals carried by the `deposit_denom` asset
     */
    deposit_decimals: number;
    /**
     * Denom for user deposits (e.g. IBC-transferred BTC)
     */
    deposit_denom: string;
    /**
     * Address of the deposit forwarder contract
     */
    deposit_forwarder_contract: string;
    /**
     * Upper limit on total AUM; deposits are rejected once the cap (if present) is exceeded
     */
    deposits_cap?: Uint128 | null;
    /**
     * Exchange rate timeout in seconds
     */
    exchange_rate_stale_period: Uint64;
    /**
     * Instantiation parameters for the fee collector.
     */
    fee_collector_params: FeeMinterParams;
    /**
     * Token factory sub-denom
     */
    maxbtc_denom: string;
    operator: string;
    owner: string;
    salt: string;
    /**
     * Address of the waitosaur observer unlocker
     */
    waitosaur_observer_unlocker: string;
    /**
     * One-off cost (Decimal) charged when a user withdraws from maxBTC
     */
    withdrawal_cost: Decimal;
}
export interface CodeIds {
    allowlist_contract_code_id: number;
    core_code_id: number;
    exchange_rate_provider_contract_code_id: number;
    fee_collector_contract_code_id: number;
    token_code_id: number;
    waitosaur_holder_contract_code_id: number;
    waitosaur_observer_contract_code_id: number;
    withdrawal_manager_contract_code_id: number;
}
/**
 * New struct to hold parameters for instantiating the fee collector contract.
 */
export interface FeeMinterParams {
    /**
     * The duration in hours for each fee collection period.
     */
    collection_period_seconds: number;
    /**
     * The percentage of APY to be taken as a fee.
     */
    fee_apy_reduction_percentage: Decimal;
}
export declare class Client {
    private readonly client;
    contractAddress: string;
    constructor(client: CosmWasmClient | SigningCosmWasmClient, contractAddress: string);
    mustBeSigningClient(): Error;
    static instantiate(client: SigningCosmWasmClient, sender: string, codeId: number, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[], admin?: string): Promise<InstantiateResult>;
    static instantiate2(client: SigningCosmWasmClient, sender: string, codeId: number, salt: Uint8Array, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[], admin?: string): Promise<InstantiateResult>;
    queryState: () => Promise<State>;
    queryOwnership: () => Promise<OwnershipForString>;
    adminExecute: (sender: string, args: AdminExecuteArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    adminExecuteMsg: (args: AdminExecuteArgs) => {
        admin_execute: AdminExecuteArgs;
    };
    updateOwnership: (sender: string, args: UpdateOwnershipArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    updateOwnershipMsg: (args: UpdateOwnershipArgs) => {
        update_ownership: UpdateOwnershipArgs;
    };
}
