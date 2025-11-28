import { CosmWasmClient, SigningCosmWasmClient, ExecuteResult, InstantiateResult } from "@cosmjs/cosmwasm-stargate";
import { StdFee } from "@cosmjs/amino";
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
 * A fixed-point decimal value with 18 fractional digits, i.e. Decimal(1_000_000_000_000_000_000) == 1.0
 *
 * The greatest possible value that can be represented is 340282366920938463463.374607431768211455 (which is (2^128 - 1) / 10^18)
 */
export type Decimal = string;
export type ContractState = "idle" | "deposit_neutron" | "deposit_pending" | "deposit_j_l_p" | "withdraw_j_l_p" | "withdraw_pending" | "withdraw_neutron";
/**
 * A fixed-point decimal value with 18 fractional digits, i.e. Decimal(1_000_000_000_000_000_000) == 1.0
 *
 * The greatest possible value that can be represented is 340282366920938463463.374607431768211455 (which is (2^128 - 1) / 10^18)
 */
export type Decimal1 = string;
export type ArrayOfBatch = Batch2[];
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
 * Actions that can be taken to alter the contract's ownership
 */
export type UpdateOwnershipArgs = {
    transfer_ownership: {
        expiry?: Expiration | null;
        new_owner: string;
    };
} | "accept_ownership" | "renounce_ownership";
export interface MaxbtcNeutronCoreSchema {
    responses: Batch | ConfigResponse | ContractState | Decimal1 | Batch1 | ArrayOfBatch | OwnershipForString | SimulateDepositResponse | Batch3;
    query: FinalizedBatchesArgs | FinalizedBatchArgs | SimulateDepositArgs;
    execute: DepositArgs | UpdateConfigArgs | MintFeeArgs | UpdateOwnershipArgs;
    instantiate?: InstantiateMsg;
    [k: string]: unknown;
}
/**
 * Each batch has a batch_id, which increments.
 */
export interface Batch {
    batch_id: number;
    /**
     * If the batch is in WITHDRAWING or FINALIZED, how much BTC was requested?
     */
    btc_requested: Uint128;
    /**
     * If in FINALIZED state, how much BTC was actually collected?
     */
    collected_amount: Uint128;
    /**
     * Historical collector balance recorded at the time the batch transitions to WITHDRAWING
     */
    collector_historical_balance: Uint128;
    /**
     * Number of decimals carried by the `deposit_denom` asset
     */
    deposit_decimals: number;
    /**
     * The amount of maxBTC burned for this batch
     */
    maxbtc_burned: Uint128;
}
/**
 * Response for querying config
 */
export interface ConfigResponse {
    deposit_cost: Decimal;
    deposit_denom: string;
    fee_collector_contract: string;
    operator: string;
    waitosaur_observer_contract: string;
    waitsaur_holder_contract: string;
    withdrawal_manager_contract: string;
}
/**
 * Each batch has a batch_id, which increments.
 */
export interface Batch1 {
    batch_id: number;
    /**
     * If the batch is in WITHDRAWING or FINALIZED, how much BTC was requested?
     */
    btc_requested: Uint128;
    /**
     * If in FINALIZED state, how much BTC was actually collected?
     */
    collected_amount: Uint128;
    /**
     * Historical collector balance recorded at the time the batch transitions to WITHDRAWING
     */
    collector_historical_balance: Uint128;
    /**
     * Number of decimals carried by the `deposit_denom` asset
     */
    deposit_decimals: number;
    /**
     * The amount of maxBTC burned for this batch
     */
    maxbtc_burned: Uint128;
}
/**
 * Each batch has a batch_id, which increments.
 */
export interface Batch2 {
    batch_id: number;
    /**
     * If the batch is in WITHDRAWING or FINALIZED, how much BTC was requested?
     */
    btc_requested: Uint128;
    /**
     * If in FINALIZED state, how much BTC was actually collected?
     */
    collected_amount: Uint128;
    /**
     * Historical collector balance recorded at the time the batch transitions to WITHDRAWING
     */
    collector_historical_balance: Uint128;
    /**
     * Number of decimals carried by the `deposit_denom` asset
     */
    deposit_decimals: number;
    /**
     * The amount of maxBTC burned for this batch
     */
    maxbtc_burned: Uint128;
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
/**
 * Each batch has a batch_id, which increments.
 */
export interface Batch3 {
    batch_id: number;
    /**
     * If the batch is in WITHDRAWING or FINALIZED, how much BTC was requested?
     */
    btc_requested: Uint128;
    /**
     * If in FINALIZED state, how much BTC was actually collected?
     */
    collected_amount: Uint128;
    /**
     * Historical collector balance recorded at the time the batch transitions to WITHDRAWING
     */
    collector_historical_balance: Uint128;
    /**
     * Number of decimals carried by the `deposit_denom` asset
     */
    deposit_decimals: number;
    /**
     * The amount of maxBTC burned for this batch
     */
    maxbtc_burned: Uint128;
}
export interface FinalizedBatchesArgs {
    limit?: number | null;
    start_after?: Uint64 | null;
}
export interface FinalizedBatchArgs {
    batch_id: number;
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
     * Admin contract with high privileges
     */
    factory_contract: string;
    /**
     * This contract is allowed to mint maxBTC to take a fee on the accrued protocol APR
     */
    fee_collector_contract: string;
    /**
     * Operator address
     */
    operator: string;
    owner: string;
    /**
     * Contract that owns and creates token factory tokens.
     */
    token_contract: string;
    /**
     * Address of the waitosaur holder contract
     */
    waitosaur_holder_contract: string;
    /**
     * Address of the waitosaur contract
     */
    waitosaur_observer_contract: string;
    /**
     * Address of the withdrawal manager contract
     */
    withdrawal_manager_contract: string;
}
export declare class Client {
    private readonly client;
    contractAddress: string;
    constructor(client: CosmWasmClient | SigningCosmWasmClient, contractAddress: string);
    mustBeSigningClient(): Error;
    static instantiate(client: SigningCosmWasmClient, sender: string, codeId: number, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[], admin?: string): Promise<InstantiateResult>;
    static instantiate2(client: SigningCosmWasmClient, sender: string, codeId: number, salt: Uint8Array, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[], admin?: string): Promise<InstantiateResult>;
    queryContractState: () => Promise<ContractState>;
    queryActiveBatch: () => Promise<Batch>;
    queryWithdrawingBatch: () => Promise<Batch>;
    queryFinalizedBatches: (args: FinalizedBatchesArgs) => Promise<ArrayOfBatch>;
    queryFinalizedBatch: (args: FinalizedBatchArgs) => Promise<Batch>;
    queryConfig: () => Promise<ConfigResponse>;
    queryExchangeRate: () => Promise<Decimal>;
    querySimulateDeposit: (args: SimulateDepositArgs) => Promise<SimulateDepositResponse>;
    queryOwnership: () => Promise<OwnershipForString>;
    tick: (sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    tickMsg: () => {
        tick: {};
    };
    deposit: (sender: string, args: DepositArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    depositMsg: (args: DepositArgs) => {
        deposit: DepositArgs;
    };
    withdraw: (sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    withdrawMsg: () => {
        withdraw: {};
    };
    updateConfig: (sender: string, args: UpdateConfigArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    updateConfigMsg: (args: UpdateConfigArgs) => {
        update_config: UpdateConfigArgs;
    };
    mintFee: (sender: string, args: MintFeeArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    mintFeeMsg: (args: MintFeeArgs) => {
        mint_fee: MintFeeArgs;
    };
    updateOwnership: (sender: string, args: UpdateOwnershipArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    updateOwnershipMsg: (args: UpdateOwnershipArgs) => {
        update_ownership: UpdateOwnershipArgs;
    };
}
