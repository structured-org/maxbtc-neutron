import { CosmWasmClient, SigningCosmWasmClient, ExecuteResult, InstantiateResult } from "@cosmjs/cosmwasm-stargate"; 
import { StdFee } from "@cosmjs/amino";
import { Coin } from "@cosmjs/amino";
/**
 * A signed fixed-point decimal value with 18 fractional digits, i.e. SignedDecimal256(1_000_000_000_000_000_000) == 1.0
 *
 * The greatest possible value that can be represented is 57896044618658097711785492504343953926634992332820282019728.792003956564819967 (which is (2^255 - 1) / 10^18) and the smallest is -57896044618658097711785492504343953926634992332820282019728.792003956564819968 (which is -2^255 / 10^18).
 */
export type SignedDecimal256 = string;

export interface MaxbtcOracleBinanceAumMockSchema {
  responses: GetDataResponse;
  instantiate?: InstantiateMsg;
  [k: string]: unknown;
}
export interface GetDataResponse {
  /**
   * The latest published data (can be null if there was no consensus reached)
   */
  last_published_data?: ConsensusOutcomeFor_BinanceData | null;
}
export interface ConsensusOutcomeFor_BinanceData {
  data: BinanceData;
  round: number;
  timestamp: number;
}
export interface BinanceData {
  pm_account_actual_equity: SignedDecimal256;
  positions: Position[];
  spot_balances: SpotBalance[];
  um_balance_usdt: SignedDecimal256;
  unimmr: SignedDecimal256;
  withdrawable_usdt: SignedDecimal256;
}
export interface Position {
  amount: SignedDecimal256;
  pnl: SignedDecimal256;
  symbol: string;
}
export interface SpotBalance {
  amount: SignedDecimal256;
  asset: string;
}
export interface InstantiateMsg {}


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
  mustBeSigningClient(): Error {
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
    admin?: string,
  ): Promise<InstantiateResult> {
    const res = await client.instantiate(sender, codeId, initMsg, label, fees, {
      ...(initCoins && initCoins.length && { funds: initCoins }), ...(admin && { admin: admin }),
    });
    return res;
  }
  static async instantiate2(
    client: SigningCosmWasmClient,
    sender: string,
    codeId: number,
    salt: Uint8Array,
    initMsg: InstantiateMsg,
    label: string,
    fees: StdFee | 'auto' | number,
    initCoins?: readonly Coin[],
    admin?: string,
  ): Promise<InstantiateResult> {
    const res = await client.instantiate2(sender, codeId, salt, initMsg, label, fees, {
      ...(initCoins && initCoins.length && { funds: initCoins }), ...(admin && { admin: admin }),
    });
    return res;
  }
  queryGetData = async(): Promise<GetDataResponse> => {
    return this.client.queryContractSmart(this.contractAddress, { get_data: {} });
  }
}
