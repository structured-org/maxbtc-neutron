import { CosmWasmClient, SigningCosmWasmClient, ExecuteResult, InstantiateResult } from "@cosmjs/cosmwasm-stargate";
import { StdFee } from "@cosmjs/amino";
/**
 * A fixed-point decimal value with 18 fractional digits, i.e. Decimal(1_000_000_000_000_000_000) == 1.0
 *
 * The greatest possible value that can be represented is 340282366920938463463.374607431768211455 (which is (2^128 - 1) / 10^18)
 */
export type Decimal = string;
/**
 * A fixed-point decimal value with 18 fractional digits, i.e. Decimal(1_000_000_000_000_000_000) == 1.0
 *
 * The greatest possible value that can be represented is 340282366920938463463.374607431768211455 (which is (2^128 - 1) / 10^18)
 */
export type Decimal1 = string;
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
 * Actions that can be taken to alter the contract's ownership
 */
export type UpdateOwnershipArgs = {
    transfer_ownership: {
        expiry?: Expiration | null;
        new_owner: string;
    };
} | "accept_ownership" | "renounce_ownership";
/**
 * Binary is a wrapper around Vec<u8> to add base64 de/serialization with serde. It also adds some helper methods to help encode inline.
 *
 * This is only needed as serde-json-{core,wasm} has a horrible encoding for Vec<u8>. See also <https://github.com/CosmWasm/cosmwasm/blob/main/docs/MESSAGE_TYPES.md>.
 */
export type Binary = string;
export interface MaxbtcNeutronCoreSchema {
    responses: ConfigResponse | Decimal1 | OwnershipForString | SimulateDepositResponse;
    query: SimulateDepositArgs;
    execute: DepositArgs | UpdateConfigArgs | MintFeeArgs | UpdateOwnershipArgs;
    instantiate?: InstantiateMsg;
    [k: string]: unknown;
}
/**
 * Response for querying config
 */
export interface ConfigResponse {
    deposit_cost: Decimal;
    deposit_denom: string;
    deposit_flush_period: number;
    fee_collector_contract: string;
    maxbtc_denom: string;
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
export interface SimulateDepositResponse {
    minted_amount: Uint128;
}
export interface SimulateDepositArgs {
    amount: Uint128;
}
export interface DepositArgs {
    min_receive_amount?: Uint128 | null;
    recipient: string;
}
/**
 * Message for updating configuration parameters (owner-only).
 */
export interface UpdateConfigArgs {
    description?: "Message for updating configuration parameters (owner-only).";
    type?: "object";
    properties?: {
        [k: string]: unknown;
    };
    additionalProperties?: never;
    required?: [];
}
export interface MintFeeArgs {
    amount: Coin;
}
export interface Coin {
    amount: Uint128;
    denom: string;
}
/**
 * InstantiateMsg configures the contract on initialization.
 */
export interface InstantiateMsg {
    /**
     * Contract address of the allow-list contract that manages the list of addresses allowed or passed KYC to mint maxBTC
     */
    allowlist_contract: string;
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
     * Minimum number of seconds that must elapse between two deposit-flush operations
     */
    deposit_flush_period: number;
    /**
     * Contract that forwards freshly-received deposits to the custody chain.
     */
    deposit_forwarder_contract: string;
    /**
     * Upper limit on total AUM; deposits are rejected once the cap (if present) is exceeded
     */
    deposits_cap?: Uint128 | null;
    /**
     * This contract provides the exchange rate for maxBTC
     */
    exchange_rate_provider_contract: string;
    /**
     * Instantiation parameters for the fee collector.
     */
    fee_collector_params: FeeMinterParams;
    /**
     * The token-factory sub-denom used for the maxBTC token
     */
    maxbtc_denom: string;
    owner: string;
}
/**
 * New struct to hold parameters for instantiating the fee collector contract.
 */
export interface FeeMinterParams {
    /**
     * The code ID of the fee collector contract wasm.
     */
    code_id: number;
    /**
     * The duration in seconds for each fee collection period.
     */
    collection_period_seconds: number;
    /**
     * The percentage of APY to be taken as a fee.
     */
    fee_apy_reduction_percentage: Decimal;
    /**
     * A unique salt for generating a predictable address with Instantiate2.
     */
    salt: Binary;
}
export declare class Client {
    private readonly client;
    contractAddress: string;
    constructor(client: CosmWasmClient | SigningCosmWasmClient, contractAddress: string);
    mustBeSigningClient(): Error;
    static instantiate(client: SigningCosmWasmClient, sender: string, codeId: number, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[]): Promise<InstantiateResult>;
    static instantiate2(client: SigningCosmWasmClient, sender: string, codeId: number, salt: number, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[]): Promise<InstantiateResult>;
    queryConfig: () => Promise<ConfigResponse>;
    queryExchangeRate: () => Promise<Decimal>;
    querySimulateDeposit: (args: SimulateDepositArgs) => Promise<SimulateDepositResponse>;
    queryOwnership: () => Promise<OwnershipForString>;
    deposit: (sender: string, args: DepositArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    flushDeposits: (sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    updateConfig: (sender: string, args: UpdateConfigArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    mintFee: (sender: string, args: MintFeeArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    updateOwnership: (sender: string, args: UpdateOwnershipArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
}
