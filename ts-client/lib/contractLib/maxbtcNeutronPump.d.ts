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
 * A human readable address.
 *
 * In Cosmos, this is typically bech32 encoded. But for multi-chain smart contracts no assumptions should be made other than being UTF-8 encoded and of reasonable length.
 *
 * This type represents a validated address. It can be created in the following ways 1. Use `Addr::unchecked(input)` 2. Use `let checked: Addr = deps.api.addr_validate(input)?` 3. Use `let checked: Addr = deps.api.addr_humanize(canonical_addr)?` 4. Deserialize from JSON. This must only be done from JSON that was validated before such as a contract's state. `Addr` must not be used in messages sent by the user because this would result in unvalidated instances.
 *
 * This type is immutable. If you really need to mutate it (Really? Are you sure?), create a mutable copy using `let mut mutable = Addr::to_string()` and operate on that `String` instance.
 */
export type Addr = string;
export interface MaxbtcNeutronPumpSchema {
    responses: Config;
    execute: PushArgs;
    instantiate?: InstantiateMsg;
    [k: string]: unknown;
}
export interface Config {
    eureka_source_channel: string;
    exact_out: boolean;
    max_fee: Coin;
    neutron_source_channel: string;
    oracle_address: Addr;
    owner: Addr;
    recover_address: string;
    relay_fee: Coin;
    source_port: string;
    to_chain_callback_contract_address: string;
    to_chain_entry_contract_address: string;
    to_chain_receiver: string;
    transfer_denom: string;
}
export interface Coin {
    amount: Uint128;
    denom: string;
}
export interface PushArgs {
    amount: Coin;
    eureka_fee: EurekaFee;
    eureka_full_timeout_nano: number;
    eureka_source_channel: string;
    to_chain_callback_contract_address: string;
    to_chain_entry_contract_address: string;
}
export interface EurekaFee {
    coin: Coin;
    receiver: string;
    timeout_timestamp: number;
}
export interface InstantiateMsg {
    eureka_fee_receiver: string;
    eureka_source_channel: string;
    exact_out: boolean;
    executor: string;
    max_fee: Coin;
    neutron_source_channel: string;
    neutron_source_port: string;
    owner: string;
    recover_address: string;
    relay_fee: Coin;
    to_chain_callback_contract_address: string;
    to_chain_entry_contract_address: string;
    transfer_denom: string;
}
export declare class Client {
    private readonly client;
    contractAddress: string;
    constructor(client: CosmWasmClient | SigningCosmWasmClient, contractAddress: string);
    mustBeSigningClient(): Error;
    static instantiate(client: SigningCosmWasmClient, sender: string, codeId: number, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[]): Promise<InstantiateResult>;
    static instantiate2(client: SigningCosmWasmClient, sender: string, codeId: number, salt: number, initMsg: InstantiateMsg, label: string, fees: StdFee | 'auto' | number, initCoins?: readonly Coin[]): Promise<InstantiateResult>;
    queryConfig: () => Promise<Config>;
    push: (sender: string, args: PushArgs, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
}
