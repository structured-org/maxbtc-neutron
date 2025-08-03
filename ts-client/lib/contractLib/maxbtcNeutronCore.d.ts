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
export interface MaxbtcNeutronCoreSchema {
    responses: ConfigResponse | Decimal1 | SimulateDepositResponse;
    query: SimulateDepositArgs;
    execute: DepositArgs | UpdateConfigArgs | MintFeeArgs;
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
    owner: string;
}
export interface SimulateDepositResponse {
    minted_amount: Uint128;
}
export interface SimulateDepositArgs {
    amount: Uint128;
}
export interface DepositArgs {
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
     * The duration in hours for each fee collection period.
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
    deposit: (sender: string, args: DepositArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    flushDeposits: (sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    updateConfig: (sender: string, args: UpdateConfigArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
    mintFee: (sender: string, args: MintFeeArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
}
