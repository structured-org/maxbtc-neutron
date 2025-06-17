import {describe, expect, it, beforeAll, afterAll} from 'vitest';
import {
    MaxbtcNeutronCore,
    MaxbtcNeutronCollector,
    MaxbtcNeutronAumOracle,
    MaxbtcNeutronLiquidationBuffer,
    MaxbtcNeutronPump
} from 'maxbtc-neutron-ts-client';

import {join} from 'path';

import {SigningCosmWasmClient} from '@cosmjs/cosmwasm-stargate';
import {Client as NeutronClient} from '@neutron-org/client-ts';
import {AccountData, DirectSecp256k1HdWallet} from '@cosmjs/proto-signing';
import {GasPrice} from '@cosmjs/stargate';
import {setupPark} from '../testSuite';
import fs from 'fs';
import Cosmopark from '@neutron-org/cosmopark';

const CoreContractClient = MaxbtcNeutronCore.Client;
const CollectorContractClient = MaxbtcNeutronCollector.Client;
const AumOracleContractClient = MaxbtcNeutronAumOracle.Client;
const LiquidationBufferContractClient = MaxbtcNeutronLiquidationBuffer.Client;
const PumpContractClient = MaxbtcNeutronPump.Client;

describe('Core', () => {
    const context: {
        park?: Cosmopark;
        wallet?: DirectSecp256k1HdWallet;
        coreContractClient?: InstanceType<typeof CoreContractClient>;
        collectorContractClient?: InstanceType<typeof CollectorContractClient>;
        aumOracleContractClient?: InstanceType<typeof AumOracleContractClient>;
        liquidationBufferContractClient?: InstanceType<typeof LiquidationBufferContractClient>;
        pumpContractClient?: InstanceType<typeof PumpContractClient>;

        account?: AccountData;
        client?: SigningCosmWasmClient;
        neutronClient?: InstanceType<typeof NeutronClient>;

        coreContractAddress?: string;
        collectorContractAddress?: string,
        aumOracleContractAddress?: string,
        liquidationBufferContractAddress?: string,
        pumpContractAddress?: string,

        treasuryAddress?: string,
    } = {};

    beforeAll(async (t) => {
        context.park = await setupPark(t, ['neutron']);
        context.wallet = await DirectSecp256k1HdWallet.fromMnemonic(
            context.park.config.wallets.demowallet1.mnemonic,
            {
                prefix: 'neutron',
            },
        );

        context.account = (await context.wallet.getAccounts())[0];
        context.neutronClient = new NeutronClient({
            apiURL: `http://127.0.0.1:${context.park.ports.neutron.rest}`,
            rpcURL: `127.0.0.1:${context.park.ports.neutron.rpc}`,
            prefix: 'neutron',
        });

        context.client = await SigningCosmWasmClient.connectWithSigner(
            `http://127.0.0.1:${context.park.ports.neutron.rpc}`,
            context.wallet,
            {
                gasPrice: GasPrice.fromString('0.025untrn'),
            },
        );

        // Random address, doesn't really matter for the tests
        context.treasuryAddress = "neutron1nxshmmwrvxa2cp80nwvf03t8u5kvl2ttr8m8f43vamudsqrdvs8qqvfwpj"
    });

    afterAll(async () => {
        await context.park.stop();
    });

    it('instantiate collector', async () => {
        const {client, account} = context;
        const res = await client.upload(
            account.address,
            Uint8Array.from(
                fs.readFileSync(
                    join(__dirname, '../../../artifacts/maxbtc_neutron_collector.wasm'),
                ),
            ),
            1.5,
        );
        expect(res.codeId).toBeGreaterThan(0);
        const instantiateRes = await MaxbtcNeutronCollector.Client.instantiate(
            client,
            account.address,
            res.codeId,
            {},
            'label',
            'auto',
            [],
        );
        expect(instantiateRes.contractAddress).toHaveLength(66);
        context.collectorContractAddress = instantiateRes.contractAddress;
        context.collectorContractClient = new MaxbtcNeutronCollector.Client(
            client,
            context.collectorContractAddress,
        );
    });

    it('instantiate aum oracle', async () => {
        const {client, account} = context;
        const res = await client.upload(
            account.address,
            Uint8Array.from(
                fs.readFileSync(
                    join(__dirname, '../../../artifacts/maxbtc_neutron_aum_oracle.wasm'),
                ),
            ),
            1.5,
        );
        expect(res.codeId).toBeGreaterThan(0);
        const instantiateRes = await MaxbtcNeutronAumOracle.Client.instantiate(
            client,
            account.address,
            res.codeId,
            {
                aum: "0"
            },
            'label',
            'auto',
            [],
        );
        expect(instantiateRes.contractAddress).toHaveLength(66);
        context.aumOracleContractAddress = instantiateRes.contractAddress;
        context.aumOracleContractClient = new MaxbtcNeutronAumOracle.Client(
            client,
            context.aumOracleContractAddress,
        );
    });

    it('instantiate liquidation buffer', async () => {
        const {client, account} = context;
        const res = await client.upload(
            account.address,
            Uint8Array.from(
                fs.readFileSync(
                    join(__dirname, '../../../artifacts/maxbtc_neutron_liquidation_buffer.wasm'),
                ),
            ),
            1.5,
        );
        expect(res.codeId).toBeGreaterThan(0);
        const instantiateRes = await MaxbtcNeutronLiquidationBuffer.Client.instantiate(
            client,
            account.address,
            res.codeId,
            {
                owned_maxbtc: "0",
                owned_btc: "0"
            },
            'label',
            'auto',
            [],
        );
        expect(instantiateRes.contractAddress).toHaveLength(66);
        context.liquidationBufferContractAddress = instantiateRes.contractAddress;
        context.liquidationBufferContractClient = new MaxbtcNeutronLiquidationBuffer.Client(
            client,
            context.liquidationBufferContractAddress,
        );
    });

    it('instantiate pump', async () => {
        const {client, account} = context;
        const res = await client.upload(
            account.address,
            Uint8Array.from(
                fs.readFileSync(
                    join(__dirname, '../../../artifacts/maxbtc_neutron_pump.wasm'),
                ),
            ),
            1.5,
        );
        expect(res.codeId).toBeGreaterThan(0);
        const instantiateRes = await MaxbtcNeutronLiquidationBuffer.Client.instantiate(
            client,
            account.address,
            res.codeId,
            {},
            'label',
            'auto',
            [],
        );
        expect(instantiateRes.contractAddress).toHaveLength(66);
        context.pumpContractAddress = instantiateRes.contractAddress;
        context.pumpContractClient = new MaxbtcNeutronPump.Client(
            client,
            context.pumpContractAddress,
        );
    });

    it('instantiate core', async () => {
        const {client, account} = context;
        const res = await client.upload(
            account.address,
            Uint8Array.from(
                fs.readFileSync(
                    join(__dirname, '../../../artifacts/maxbtc_neutron_core.wasm'),
                ),
            ),
            1.5,
        );
        expect(res.codeId).toBeGreaterThan(0);
        const instantiateRes = await MaxbtcNeutronCore.Client.instantiate(
            client,
            account.address,
            res.codeId,
            {
                aum_contract: context.aumOracleContractAddress,
                collector_contract: context.collectorContractAddress,
                treasury_address: context.treasuryAddress,
                liquidation_buffer_contract: context.liquidationBufferContractAddress,
                deposit_pump_contract: context.pumpContractAddress,
                accepted_withdrawable_percentage: "0.005",
                batch_active_duration: 10,
                batch_withdrawing_duration: 10,
                cached_aum_tolerance: "0.02",
                cached_er_ttl: 20,
                deposit_decimals: 6,
                deposit_denom: "untrn",
                deposit_fee: "0.01",
                deposit_flush_period: 10,
                liquidation_buffer_share: "0.1",
                maxbtc_denom: "maxbtc",
                owner: account.address,
            },
            'label',
            'auto',
            [],
        );
        expect(instantiateRes.contractAddress).toHaveLength(66);
        context.coreContractAddress = instantiateRes.contractAddress;
        context.coreContractClient = new MaxbtcNeutronCore.Client(
            client,
            context.coreContractAddress,
        );
    });

    it('Deposit first time and check maxBTC balance', async () => {
      const { neutronClient, coreContractClient, account, coreContractAddress } = context;
      let res1 = await coreContractClient.deposit(
        account.address,
        {
          recipient: account.address,
        },
        1.5,
        "memo",
        [{
            denom: "untrn",
            amount: "100"
        }]
      );
      expect(res1.transactionHash).toBeTruthy();

      let res2 =
            await neutronClient.CosmosBankV1Beta1.query.queryBalance(
                account.address,
                { denom: `factory/${coreContractAddress}/maxbtc` },
            );
      expect(res2.data.balance.amount).toEqual('99');
    });

    it('Deposit second time and check maxBTC balance', async () => {
        const { neutronClient, coreContractClient, account, coreContractAddress } = context;
        let res1 = await coreContractClient.deposit(
            account.address,
            {
                recipient: account.address,
            },
            1.5,
            "memo",
            [{
                denom: "untrn",
                amount: "100"
            }]
        );
        expect(res1.transactionHash).toBeTruthy();

        let res2 =
            await neutronClient.CosmosBankV1Beta1.query.queryBalance(
                account.address,
                { denom: `factory/${coreContractAddress}/maxbtc` },
            );
        // Not 198 because after first mint, the ER is 1.01 due to the fee we took
        expect(res2.data.balance.amount).toEqual('197');
    });
});
