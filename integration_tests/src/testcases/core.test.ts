import {describe, expect, it, beforeAll, afterAll} from 'vitest';
import {
    MaxbtcNeutronCore,
    MaxbtcNeutronCollector,
    MaxbtcNeutronAumOracle,
    MaxbtcNeutronLiquidationBuffer,
} from 'maxbtc-neutron-ts-client';

import {join} from 'path';

import {SigningCosmWasmClient} from '@cosmjs/cosmwasm-stargate';
import {Client as NeutronClient} from '@neutron-org/client-ts';
import {AccountData, DirectSecp256k1HdWallet} from '@cosmjs/proto-signing';
import {GasPrice, Coin, StdFee} from '@cosmjs/stargate';
import {setupPark} from '../testSuite';
import fs from 'fs';
import Cosmopark from '@neutron-org/cosmopark';
import {waitForTx} from "../helpers/waitForTx";

const CoreContractClient = MaxbtcNeutronCore.Client;
const CollectorContractClient = MaxbtcNeutronCollector.Client;
const AumOracleContractClient = MaxbtcNeutronAumOracle.Client;
const LiquidationBufferContractClient = MaxbtcNeutronLiquidationBuffer.Client;

describe('Core', () => {
    const context: {
        park?: Cosmopark;
        wallet?: DirectSecp256k1HdWallet;
        coreContractClient?: InstanceType<typeof CoreContractClient>;
        collectorContractClient?: InstanceType<typeof CollectorContractClient>;
        aumOracleContractClient?: InstanceType<typeof AumOracleContractClient>;
        liquidationBufferContractClient?: InstanceType<typeof LiquidationBufferContractClient>;

        account?: AccountData;
        client?: SigningCosmWasmClient;
        neutronClient?: InstanceType<typeof NeutronClient>;

        coreContractAddress?: string;
        collectorContractAddress?: string,
        aumOracleContractAddress?: string,
        liquidationBufferContractAddress?: string,
        pumpContractAddress?: string,
        pumpLibraryContractAddress?: string,

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

    it('instantiate pump (valence base account)', async () => {
        const {client, account} = context;
        const res = await client.upload(
            account.address,
            Uint8Array.from(
                fs.readFileSync(
                    join(__dirname, '../../../artifacts/valence_base_account.wasm'),
                ),
            ),
            1.5,
        );
        expect(res.codeId).toBeGreaterThan(0);

        const instantiateMsg = {
            admin: account.address,
            approved_libraries: []
        };

        const instantiateRes = await client.instantiate(
            account.address,
            res.codeId,
            instantiateMsg,
            'label',
            'auto',
        );

        expect(instantiateRes.contractAddress).toBeTruthy();
        expect(instantiateRes.contractAddress).toHaveLength(66);
        context.pumpContractAddress = instantiateRes.contractAddress;
    });

    it('instantiate pump library (valence ibc transfer library)', async () => {
        const {client, account} = context;
        const res = await client.upload(
            account.address,
            Uint8Array.from(
                fs.readFileSync(
                    join(__dirname, '../../../artifacts/valence_neutron_ibc_transfer_library.wasm'),
                ),
            ),
            1.5,
        );
        expect(res.codeId).toBeGreaterThan(0);

        const instantiateMsg = {
            owner: account.address,
            processor: account.address,
            config: {
                input_addr: { library_account_addr: context.pumpContractAddress },
                output_addr: { library_account_addr: "0x1234567890123456789012345678901234567890" },
                denom: { native: "ibc/0E293A7622DC9A6439DB60E6D234B5AF446962E27CA3AB44D0590603DFF6968E" },
                amount: "full_amount",
                memo: "",
                remote_chain_info: {
                    channel_id: "channel-1",
                },
                denom_to_pfm_map: {},
                eureka_config: {
                    callback_contract: "cosmos1lqu9662kd4my6dww4gzp3730vew0gkwe0nl9ztjh0n5da0a8zc4swsvd22",
                    action_contract: "cosmos1clswlqlfm8gpn7n5wu0ypu0ugaj36urlhj7yz30hn7v7mkcm2tuqy9f8s5",
                    recover_address: "cosmos1ep2umj6kn34g2ttjalsc5r9w8pt7sv4x9z0q26",
                    source_channel: "08-wasm-1369"
                },
            },
        };

        const instantiateRes = await client.instantiate(
            account.address,
            res.codeId,
            instantiateMsg,
            'label',
            'auto',
        );

        expect(instantiateRes.contractAddress).toBeTruthy();
        expect(instantiateRes.contractAddress).toHaveLength(66);
        context.pumpLibraryContractAddress = instantiateRes.contractAddress;

        const approveRes = await client.execute(
            account.address,
            context.pumpContractAddress,
            {
                approve_library: {
                    library: context.pumpLibraryContractAddress,
                }
            },
            'auto',
        )
        expect(approveRes.transactionHash).toHaveLength(64);
        await waitForTx(client, approveRes.transactionHash);
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
            'auto',
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
            'auto',
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
        // Not 198 because after first mint, the ER is ~1.01 due to the fee we took
        expect(res2.data.balance.amount).toEqual('197');
    });

    it('should handle the full withdrawal cycle: withdraw -> process -> fund collector -> claim', async () => {
        const { coreContractClient, client, account, coreContractAddress, collectorContractAddress } = context;

        // The previous tests left the user with 197 maxBTC. We'll deposit more for a more robust test.
        console.log("Seeding account with additional maxBTC for withdrawal tests...");
        const depositRes = await coreContractClient.deposit(
            account.address,
            { recipient: account.address },
            'auto',
            "seeding deposit for cycle test",
            [{ denom: "untrn", amount: "200000" }]
        );
        await waitForTx(client, depositRes.transactionHash);

        const withdrawAmount = "75000";

        // STEP 1: Withdraw (Burn maxBTC to get redemption tokens)
        console.log(`Withdrawing ${withdrawAmount} maxBTC...`);
        const initialMaxBtcBalance = BigInt((await client.getBalance(account.address, `factory/${coreContractAddress}/maxbtc`)).amount);

        const withdrawRes = await coreContractClient.withdraw(
            account.address,
            'auto',
            "initiating withdrawal",
            [{ denom: `factory/${coreContractAddress}/maxbtc`, amount: withdrawAmount }]
        );
        expect(withdrawRes.transactionHash).toBeTruthy();
        await waitForTx(client, withdrawRes.transactionHash);

        // Assertions for withdraw
        const finalMaxBtcBalance = BigInt((await client.getBalance(account.address, `factory/${coreContractAddress}/maxbtc`)).amount);
        expect(finalMaxBtcBalance).toEqual(initialMaxBtcBalance - BigInt(withdrawAmount));

        const activeBatch = await coreContractClient.queryActiveBatch();
        expect(activeBatch.maxbtc_burned).toEqual(withdrawAmount);
        const redemptionTokenDenom = `factory/${coreContractAddress}/redemption/batch/${activeBatch.batch_id}`;

        const redemptionTokenBalance = (await client.getBalance(account.address, redemptionTokenDenom)).amount;
        expect(redemptionTokenBalance).toEqual(withdrawAmount);
        console.log(`Successfully withdrew and received ${withdrawAmount} of ${redemptionTokenDenom}`);

        // STEP 2: Wait for batch to be processable
        const batchActiveDuration = 10; // As set in instantiation
        console.log(`Waiting for ${batchActiveDuration + 1} seconds for the batch to expire...`);
        await new Promise(resolve => setTimeout(resolve, (batchActiveDuration + 1) * 1000));

        // STEP 3: Process the active batch
        console.log("Processing the active batch...");
        const processRes = await coreContractClient.processActiveBatch(account.address, 'auto');
        expect(processRes.transactionHash).toBeTruthy();
        await waitForTx(client, processRes.transactionHash);

        // Assertions for process batch
        const withdrawingBatch = await coreContractClient.queryWithdrawingBatch();
        expect(withdrawingBatch).toBeTruthy();
        expect(withdrawingBatch.batch_id).toEqual(activeBatch.batch_id);
        expect(withdrawingBatch.maxbtc_burned).toEqual(withdrawAmount);
        expect(Number(withdrawingBatch.btc_requested)).toBeGreaterThan(0);
        console.log(`Batch ${activeBatch.batch_id} is now in 'withdrawing' state, requesting ${withdrawingBatch.btc_requested} untrn.`);

        // STEP 4: Simulate Collector Funding & Finalize Batch
        const amountNeeded = withdrawingBatch.btc_requested;
        console.log(`Sending ${amountNeeded} untrn to collector contract ${collectorContractAddress}...`);
        const fundCollectorRes = await client.sendTokens(
            account.address,
            collectorContractAddress,
            [{ denom: 'untrn', amount: amountNeeded }],
            'auto'
        );
        await waitForTx(client, fundCollectorRes.transactionHash);
        console.log("Collector funded.");

        // Trigger _process_cache to finalize the batch. Any execute message will do.
        // We will make a small, harmless deposit.
        console.log("Triggering _process_cache to finalize the batch...");
        await coreContractClient.deposit(account.address, { recipient: account.address }, 'auto', "trigger cache", [{denom: 'untrn', amount: '1'}]);

        // Assertions for finalization
        const finalizedBatch = await coreContractClient.queryFinalizedBatch({ batch_id: withdrawingBatch.batch_id });
        expect(finalizedBatch).toBeTruthy();
        expect(finalizedBatch.collected_amount).toEqual(amountNeeded);
        const postFinalizeWithdrawingBatch = await coreContractClient.queryWithdrawingBatch();
        expect(postFinalizeWithdrawingBatch).toBeNull();
        console.log(`Batch ${withdrawingBatch.batch_id} is finalized.`);

        // STEP 5: Claim underlying asset
        console.log(`Claiming ${amountNeeded} untrn from finalized batch ${finalizedBatch.batch_id}...`);
        const claimRes = await coreContractClient.claim(
            account.address,
            { recipient: account.address },
            'auto',
            "claiming my withdrawal",
            [{ denom: redemptionTokenDenom, amount: withdrawAmount }]
        );
        expect(claimRes.transactionHash).toBeTruthy();
        await waitForTx(client, claimRes.transactionHash);

        // Assertions for claim
        const finalRedemptionTokenBalance = (await client.getBalance(account.address, redemptionTokenDenom)).amount;
        expect(finalRedemptionTokenBalance).toEqual('0');

        const tx = await client.getTx(claimRes.transactionHash);
        const transferEvent = tx.events.find(e => e.type === 'transfer' && e.attributes.some(attr => attr.key === 'recipient' && attr.value === account.address));
        expect(transferEvent).toBeDefined();
        const receivedAmountAttr = transferEvent.attributes.find(attr => attr.key === 'amount');
        expect(receivedAmountAttr.value).toContain(amountNeeded);
        console.log("Claim successful. Full cycle complete.");
    });

    it('should handle flushDeposits and FSM state transitions correctly', async () => {
        const { client, coreContractClient, account, coreContractAddress, liquidationBufferContractAddress, pumpContractAddress, aumOracleContractClient } = context;

        // Ensure core contract has funds.
        console.log("Depositing funds to prepare for flush test...");
        await coreContractClient.deposit(account.address, { recipient: account.address }, 'auto', "flush setup", [{ denom: "untrn", amount: "100000" }]);

        // STEP 1: Wait for flush period
        const flushPeriod = 10; // As set in instantiation
        console.log(`Waiting for ${flushPeriod + 1} seconds for flush period...`);
        await new Promise(resolve => setTimeout(resolve, (flushPeriod + 1) * 1000));

        // Get pre-flush state
        const coreBalanceBefore = await client.getBalance(coreContractAddress, 'untrn');
        const bufferBalanceBefore = await client.getBalance(liquidationBufferContractAddress, 'untrn');
        const pumpBalanceBefore = await client.getBalance(pumpContractAddress, 'untrn');

        // Mock external state for predictable calculation
        await aumOracleContractClient.updateConfig(account.address, { aum: "0" }, 'auto');
        await context.liquidationBufferContractClient.updateConfig(account.address, {owned_btc: "0", owned_maxbtc: "0"}, 'auto');

        // Calculation based on contract logic
        const totalAUM = BigInt(coreBalanceBefore.amount) + BigInt(bufferBalanceBefore.amount);
        const requiredBuffer = totalAUM / 10n; // liquidation_buffer_share is "0.1"
        const toBuffer = requiredBuffer - BigInt(bufferBalanceBefore.amount);
        const toPump = BigInt(coreBalanceBefore.amount) - toBuffer;

        // STEP 2: Execute flushDeposits
        console.log("Flushing deposits...");
        await coreContractClient.flushDeposits(account.address, 'auto');

        // Assertions for fund movement
        const coreBalanceAfter = await client.getBalance(coreContractAddress, 'untrn');
        const bufferBalanceAfter = await client.getBalance(liquidationBufferContractAddress, 'untrn');
        const pumpBalanceAfter = await client.getBalance(pumpContractAddress, 'untrn');

        expect(coreBalanceAfter.amount).toEqual("0");
        expect(BigInt(bufferBalanceAfter.amount)).toEqual(BigInt(bufferBalanceBefore.amount) + toBuffer);
        expect(BigInt(pumpBalanceAfter.amount)).toEqual(BigInt(pumpBalanceBefore.amount) + toPump);
        console.log(`Flush successful. Sent ${toBuffer} to buffer, ${toPump} to pump.`);

        // Assert FSM is in 'Flushing' state by attempting another state-changing call, which should fail.
        await expect(coreContractClient.processActiveBatch(account.address, 'auto')).rejects.toThrow(/Contract is not idle/);
        console.log("FSM is in 'Flushing' state as expected.");

        // STEP 3: Finalize the Flush
        // Simulate the off-chain process by updating the AUM oracle with the amount sent to the pump.
        console.log(`Updating AUM oracle with ${toPump.toString()}...`);
        await aumOracleContractClient.updateConfig(account.address, { aum: toPump.toString() }, 'auto');

        // Trigger _process_cache again. The contract should see the AUM updated and return to Idle.
        // We will try `processActiveBatch` again. It will fail for its own reasons, but _process_cache runs first.
        try {
            await coreContractClient.processActiveBatch(account.address, 'auto');
        } catch (e) {
            // This is expected because the batch duration has not passed.
            // The _process_cache hook should have executed and returned the FSM to Idle.
        }

        // Assert FSM is 'Idle' by successfully executing a state-changing call.
        const finalDepositRes = await coreContractClient.deposit(account.address, { recipient: account.address }, 'auto', "testing idle state", [{ denom: "untrn", amount: "1" }]);
        expect(finalDepositRes.transactionHash).toBeTruthy();
        console.log("FSM is back to 'Idle', flush is fully finalized.");
    });
});