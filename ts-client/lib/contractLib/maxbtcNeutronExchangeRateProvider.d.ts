import {
  CosmWasmClient,
  SigningCosmWasmClient,
  ExecuteResult,
  InstantiateResult,
} from '@cosmjs/cosmwasm-stargate';
import { StdFee } from '@cosmjs/amino';
import { Coin } from '@cosmjs/amino';
/**
 * An implementation of i256 that is using strings for JSON encoding/decoding, such that the full i256 range can be used for clients that convert JSON numbers to floats, like JavaScript and jq.
 *
 * # Examples
 *
 * Use `from` to create instances out of primitive uint types or `new` to provide big endian bytes:
 *
 * ``` # use cosmwasm_std::Int256; let a = Int256::from(258u128); let b = Int256::new([ 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 1u8, 2u8, ]); assert_eq!(a, b); ```
 */
export type Int256 = string;
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
export type Expiration =
  | {
      at_height: number;
    }
  | {
      at_time: Timestamp;
    }
  | {
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
export type UpdateOwnershipArgs =
  | {
      transfer_ownership: {
        expiry?: Expiration | null;
        new_owner: string;
      };
    }
  | 'accept_ownership'
  | 'renounce_ownership';
export interface MaxbtcNeutronExchangeRateProviderSchema {
  responses: Decimal | GetTwaerResponse | OwnershipForString;
  execute: UpdateExchangeRateArgs | UpdateOwnershipArgs;
  instantiate?: InstantiateMsg;
  [k: string]: unknown;
}
/**
 * Response type for AUM Oracle receiver GetAum queries. **Must** return the AUM in micro-Bitcoin (uwBTC) with precision of `WBTC_DECIMALS`.
 */
export interface GetAumResponse {
  /**
   * The latest AUM in the AUM Oracle receiver reported by messengers scaled to `WBTC_DECIMALS`.
   */
  aum_in_wbtc: Int256;
  /**
   * Represents the number of decimals that the `aum_in_wbtc` is represented in. Must always equal to `WBTC_DECIMALS`. It is used to scale the `aum_in_wbtc` to its base BTC value. E.g. `base_aum_in_btc = aum_in_wbtc / 10^decimals`.
   */
  decimals: number;
}
export interface GetTwaerResponse {
  published_at: number;
  twaer: Decimal1;
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
export interface UpdateExchangeRateArgs {
  rate: Decimal1;
}
export interface UpdateAumArgs {
  aum_in_wbtc: Int256;
  decimals: number;
}
export interface InstantiateMsg {
  owner: string;
}
export declare class Client {
  private readonly client;
  contractAddress: string;
  constructor(
    client: CosmWasmClient | SigningCosmWasmClient,
    contractAddress: string,
  );
  mustBeSigningClient(): Error;
  static instantiate(
    client: SigningCosmWasmClient,
    sender: string,
    codeId: number,
    initMsg: InstantiateMsg,
    label: string,
    fees: StdFee | 'auto' | number,
    initCoins?: readonly Coin[],
    admin?: string,
  ): Promise<InstantiateResult>;
  static instantiate2(
    client: SigningCosmWasmClient,
    sender: string,
    codeId: number,
    salt: Uint8Array,
    initMsg: InstantiateMsg,
    label: string,
    fees: StdFee | 'auto' | number,
    initCoins?: readonly Coin[],
    admin?: string,
  ): Promise<InstantiateResult>;
  queryGetTwaer: () => Promise<GetTwaerResponse>;
  queryExchangeRate: () => Promise<Decimal>;
  queryOwnership: () => Promise<OwnershipForString>;
  updateExchangeRate: (
    sender: string,
    args: UpdateExchangeRateArgs,
    fee?: number | StdFee | 'auto',
    memo?: string,
    funds?: Coin[],
  ) => Promise<ExecuteResult>;
  updateExchangeRateMsg: (args: UpdateExchangeRateArgs) => {
    update_exchange_rate: UpdateExchangeRateArgs;
  };
  updateAum: (
    sender: string,
    args: UpdateAumArgs,
    fee?: number | StdFee | 'auto',
    memo?: string,
    funds?: Coin[],
  ) => Promise<ExecuteResult>;
  updateAumMsg: (args: UpdateAumArgs) => {
    update_aum: UpdateAumArgs;
  };
  updateOwnership: (
    sender: string,
    args: UpdateOwnershipArgs,
    fee?: number | StdFee | 'auto',
    memo?: string,
    funds?: Coin[],
  ) => Promise<ExecuteResult>;
  updateOwnershipMsg: (args: UpdateOwnershipArgs) => {
    update_ownership: UpdateOwnershipArgs;
  };
}
