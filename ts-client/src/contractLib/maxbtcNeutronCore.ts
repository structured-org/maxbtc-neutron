import { CosmWasmClient, SigningCosmWasmClient, ExecuteResult, InstantiateResult } from "@cosmjs/cosmwasm-stargate"; 
import { StdFee } from "@cosmjs/amino";
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
/**
 * Binary is a wrapper around Vec<u8> to add base64 de/serialization with serde. It also adds some helper methods to help encode inline.
 *
 * This is only needed as serde-json-{core,wasm} has a horrible encoding for Vec<u8>. See also <https://github.com/CosmWasm/cosmwasm/blob/main/docs/MESSAGE_TYPES.md>.
 */
export type Binary = string;

export interface MaxbtcNeutronCoreSchema {
  responses:
    | NullableBatchResponse
    | ConfigResponse
    | ContractState
    | Decimal1
    | NullableBatchResponse1
    | NullableBatchResponse2;
  query: FinalizedBatchArgs;
  execute: DepositArgs | ClaimArgs | UpdateConfigArgs | MintFeeArgs;
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
   * Instantiation parameters for the fee collector.
   */
  fee_collector_params: FeeMinterParams;
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


function isSigningCosmWasmClient(
  client: CosmWasmClient | SigningCosmWasmClient
): client is SigningCosmWasmClient {
  return 'execute' in client;
}

export class Client {
  private readonly client: CosmWasmClient | SigningCosmWasmClient;
  contractAddress: string;
  constructor(client: CosmWasmClient | SigningCosmWasmClient, contractAddress: string) {
    this.client = client;
    this.contractAddress = contractAddress;
  }
  mustBeSigningClient() {
    return new Error("This client is not a SigningCosmWasmClient");
  }
  static async instantiate(
    client: SigningCosmWasmClient,
    sender: string,
    codeId: number,
    initMsg: InstantiateMsg,
    label: string,
    fees: StdFee | 'auto' | number,
    initCoins?: readonly Coin[],
  ): Promise<InstantiateResult> {
    const res = await client.instantiate(sender, codeId, initMsg, label, fees, {
      ...(initCoins && initCoins.length && { funds: initCoins }),
    });
    return res;
  }
  static async instantiate2(
    client: SigningCosmWasmClient,
    sender: string,
    codeId: number,
    salt: number,
    initMsg: InstantiateMsg,
    label: string,
    fees: StdFee | 'auto' | number,
    initCoins?: readonly Coin[],
  ): Promise<InstantiateResult> {
    const res = await client.instantiate2(sender, codeId, new Uint8Array([salt]), initMsg, label, fees, {
      ...(initCoins && initCoins.length && { funds: initCoins }),
    });
    return res;
  }
  queryConfig = async(): Promise<ConfigResponse> => {
    return this.client.queryContractSmart(this.contractAddress, { config: {} });
  }
  queryActiveBatch = async(): Promise<NullableBatchResponse> => {
    return this.client.queryContractSmart(this.contractAddress, { active_batch: {} });
  }
  queryWithdrawingBatch = async(): Promise<NullableBatchResponse> => {
    return this.client.queryContractSmart(this.contractAddress, { withdrawing_batch: {} });
  }
  queryFinalizedBatch = async(args: FinalizedBatchArgs): Promise<NullableBatchResponse> => {
    return this.client.queryContractSmart(this.contractAddress, { finalized_batch: args });
  }
  queryContractState = async(): Promise<ContractState> => {
    return this.client.queryContractSmart(this.contractAddress, { contract_state: {} });
  }
  queryExchangeRate = async(): Promise<Decimal> => {
    return this.client.queryContractSmart(this.contractAddress, { exchange_rate: {} });
  }
  deposit = async(sender:string, args: DepositArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> =>  {
          if (!isSigningCosmWasmClient(this.client)) { throw this.mustBeSigningClient(); }
    return this.client.execute(sender, this.contractAddress, { deposit: args }, fee || "auto", memo, funds);
  }
  flushDeposits = async(sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> =>  {
          if (!isSigningCosmWasmClient(this.client)) { throw this.mustBeSigningClient(); }
    return this.client.execute(sender, this.contractAddress, { flush_deposits: {} }, fee || "auto", memo, funds);
  }
  withdraw = async(sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> =>  {
          if (!isSigningCosmWasmClient(this.client)) { throw this.mustBeSigningClient(); }
    return this.client.execute(sender, this.contractAddress, { withdraw: {} }, fee || "auto", memo, funds);
  }
  processActiveBatch = async(sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> =>  {
          if (!isSigningCosmWasmClient(this.client)) { throw this.mustBeSigningClient(); }
    return this.client.execute(sender, this.contractAddress, { process_active_batch: {} }, fee || "auto", memo, funds);
  }
  claim = async(sender:string, args: ClaimArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> =>  {
          if (!isSigningCosmWasmClient(this.client)) { throw this.mustBeSigningClient(); }
    return this.client.execute(sender, this.contractAddress, { claim: args }, fee || "auto", memo, funds);
  }
  processCache = async(sender: string, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> =>  {
          if (!isSigningCosmWasmClient(this.client)) { throw this.mustBeSigningClient(); }
    return this.client.execute(sender, this.contractAddress, { process_cache: {} }, fee || "auto", memo, funds);
  }
  updateConfig = async(sender:string, args: UpdateConfigArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> =>  {
          if (!isSigningCosmWasmClient(this.client)) { throw this.mustBeSigningClient(); }
    return this.client.execute(sender, this.contractAddress, { update_config: args }, fee || "auto", memo, funds);
  }
  mintFee = async(sender:string, args: MintFeeArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> =>  {
          if (!isSigningCosmWasmClient(this.client)) { throw this.mustBeSigningClient(); }
    return this.client.execute(sender, this.contractAddress, { mint_fee: args }, fee || "auto", memo, funds);
  }
}
