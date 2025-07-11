import { CosmWasmClient, SigningCosmWasmClient, ExecuteResult, InstantiateResult } from "@cosmjs/cosmwasm-stargate";
import { StdFee } from "@cosmjs/amino";
import { Coin } from "@cosmjs/amino";
export type NullableBatchResponse = BatchResponse | null;
/**
 * A fixed-point decimal value with 18 fractional digits, i.e. Decimal(1_000_000_000_000_000_000) == 1.0
 *
 * The greatest possible value that can be represented is 340282366920938463463.374607431768211455 (which is (2^128 - 1) / 10^18)
 */
export type Decimal = string;
/**
 * Represents the contract state.
 */
export type ContractState = "idle" | "flushing" | "withdrawing";
/**
 * A fixed-point decimal value with 18 fractional digits, i.e. Decimal(1_000_000_000_000_000_000) == 1.0
 *
 * The greatest possible value that can be represented is 340282366920938463463.374607431768211455 (which is (2^128 - 1) / 10^18)
 */
export type Decimal1 = string;
export type NullableBatchResponse1 = BatchResponse | null;
export type NullableBatchResponse2 = BatchResponse | null;
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
export interface MaxbtcNeutronCoreSchema {
    responses: NullableBatchResponse | ConfigResponse | ContractState | Decimal1 | NullableBatchResponse1 | NullableBatchResponse2;
    query: FinalizedBatchArgs;
    execute: DepositArgs | ClaimArgs | UpdateConfigArgs;
    instantiate?: InstantiateMsg;
    [k: string]: unknown;
}
/**
 * Response for batch query
 */
export interface BatchResponse {
    batch_id: number;
    btc_requested: string;
    collected_amount: string;
    collector_historical_balance: string;
    maxbtc_burned: string;
}
/**
 * Response for querying config
 */
export interface ConfigResponse {
    accepted_withdrawable_percentage: Decimal;
    aum_contract: string;
    batch_active_duration: number;
    batch_withdrawing_duration: number;
    deposit_cost: Decimal;
    deposit_denom: string;
    deposit_flush_period: number;
    liquidation_buffer_share: Decimal;
    liquidation_contract: string;
    maxbtc_denom: string;
    owner: string;
    treasury_address: string;
}
export interface FinalizedBatchArgs {
    batch_id: number;
}
export interface DepositArgs {
    recipient: string;
}
export interface ClaimArgs {
    /**
     * The user wants to receive BTC at `recipient` address on Neutron (which the user can IBC-transfer out later).
     */
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
/**
 * InstantiateMsg configures the contract on initialization.
 */
export interface InstantiateMsg {
    /**
     * Minimum percentage (Decimal) of `btc_requested` that must be collected for a batch to finalize successfully
     */
    accepted_withdrawable_percentage: Decimal;
    aum_contract: string;
    /**
     * Number of seconds an ACTIVE batch remains open before it can be promoted to WITHDRAWING
     */
    batch_active_duration: number;
    /**
     * Number of seconds a WITHDRAWING batch may remain open before it must be finalized
     */
    batch_withdrawing_duration: number;
    /**
     * Maximum tolerated relative difference (Decimal) between the deposit buffer sent for flushing and the amount observed on the custody chain
     */
    cached_aum_tolerance: Decimal;
    /**
     * Lifetime, in seconds, of the cached ER/AUM snapshot that protects the protocol while a multi-step operation is in flight
     */
    cached_er_ttl: number;
    /**
     * Collector contract that receives BTC shipped back from custody during the withdrawal process.
     */
    collector_contract: string;
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
    deposit_pump_contract: string;
    /**
     * Optional allow-list of addresses that may mint maxBTC while the list is active (empty or `None` means open to everyone)
     */
    deposits_allowlist?: string[] | null;
    /**
     * Upper limit on total AUM; deposits are rejected once the cap (if present) is exceeded
     */
    deposits_cap?: Uint128 | null;
    /**
     * Address of the contract that manages the liquidation buffer.
     */
    liquidation_buffer_contract: string;
    /**
     * Fraction of total AUM (Decimal) that the protocol keeps on the liquidation buffer contract as an instant-liquidity buffer
     */
    liquidation_buffer_share: Decimal;
    /**
     * The token-factory sub-denom used for the maxBTC token
     */
    maxbtc_denom: string;
    owner: string;
    /**
     * Treasury account that receives protocol fees and surplus funds.
     */
    treasury_address: string;
}
export declare class Client {
    private readonly client;
    contractAddress: string;
    constructor(client: CosmWasmClient | SigningCosmWasmClient, contractAddress: string);
    mustBeSigningClient(): Error;
    static instantiate(client: SigningCosmWasmClient, sender: string, codeId: number, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[]): Promise<InstantiateResult>;
    static instantiate2(client: SigningCosmWasmClient, sender: string, codeId: number, salt: number, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[]): Promise<InstantiateResult>;
    queryConfig: () => Promise<ConfigResponse>;
    queryActiveBatch: () => Promise<NullableBatchResponse>;
    queryWithdrawingBatch: () => Promise<NullableBatchResponse>;
    queryFinalizedBatch: (args: FinalizedBatchArgs) => Promise<NullableBatchResponse>;
    queryContractState: () => Promise<ContractState>;
    queryExchangeRate: () => Promise<Decimal>;
    deposit: (sender: string, args: DepositArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    flushDeposits: (sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    withdraw: (sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    processActiveBatch: (sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    claim: (sender: string, args: ClaimArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    processCache: (sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    updateConfig: (sender: string, args: UpdateConfigArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
}
