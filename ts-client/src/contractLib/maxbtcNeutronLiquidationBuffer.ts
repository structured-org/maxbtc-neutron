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
export type Uint1281 = string;
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
export type Uint1282 = string;

export interface MaxbtcNeutronLiquidationBufferSchema {
  responses: Uint128 | Uint1281;
  execute: ClawBackArgs;
  instantiate?: InstantiateMsg;
  [k: string]: unknown;
}
export interface ClawBackArgs {
  amount: Coin;
}
export interface Coin {
  amount: Uint1282;
  denom: string;
}
export interface InstantiateMsg {
  owned_btc?: Uint1282 | null;
  owned_maxbtc?: Uint1282 | null;
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
  queryGetBTCBalance = async(): Promise<Uint128> => {
    return this.client.queryContractSmart(this.contractAddress, { get_b_t_c_balance: {} });
  }
  queryGetMaxBTCBalance = async(): Promise<Uint128> => {
    return this.client.queryContractSmart(this.contractAddress, { get_max_b_t_c_balance: {} });
  }
  clawBack = async(sender:string, args: ClawBackArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> =>  {
          if (!isSigningCosmWasmClient(this.client)) { throw this.mustBeSigningClient(); }
    return this.client.execute(sender, this.contractAddress, { claw_back: args }, fee || "auto", memo, funds);
  }
}
