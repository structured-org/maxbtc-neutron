import { CosmWasmClient, SigningCosmWasmClient, InstantiateResult } from "@cosmjs/cosmwasm-stargate";
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
    /**
     * The data submitted by the messengers
     */
    data: BinanceData;
}
export interface BinanceData {
    spot_balances: SpotBalance[];
}
export interface SpotBalance {
    amount: SignedDecimal256;
    asset: string;
}
export interface InstantiateMsg {
}
export declare class Client {
    private readonly client;
    contractAddress: string;
    constructor(client: CosmWasmClient | SigningCosmWasmClient, contractAddress: string);
    mustBeSigningClient(): Error;
    static instantiate(client: SigningCosmWasmClient, sender: string, codeId: number, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[], admin?: string): Promise<InstantiateResult>;
    static instantiate2(client: SigningCosmWasmClient, sender: string, codeId: number, salt: Uint8Array, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[], admin?: string): Promise<InstantiateResult>;
    queryGetData: () => Promise<GetDataResponse>;
}
