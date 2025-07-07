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
import {GasPrice} from '@cosmjs/stargate';
import {setupPark} from '../testSuite';
import fs from 'fs';
import Cosmopark from '@neutron-org/cosmopark';
import {waitForTx} from "../helpers/waitForTx";
import {sleep} from "../helpers/sleep";

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
                deposit_cost: "0.01",
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

        await sleep(10000);

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
            1.5
        );
        await waitForTx(client, fundCollectorRes.transactionHash);
        console.log("Collector funded.");

        // Trigger _process_cache to finalize the batch. Any execute message will do.
        // We will make a small, harmless deposit.
        console.log("Triggering _process_cache to finalize the batch...");
        await coreContractClient.processCache(account.address, 1.5, "process cache");

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

        // Get pre-flush state.
        const coreBalanceBefore = await client.getBalance(coreContractAddress, 'untrn');
        const bufferBalanceBefore = await client.getBalance(liquidationBufferContractAddress, 'untrn');
        const pumpBalanceBefore = await client.getBalance(pumpContractAddress, 'untrn');

        // Mock external state for predictable calculation. Note: bufferBalanceBefore is expected to be
        // 0, so we are just making sure that the mock contract doesn't hallucinate a different value
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

        expect(await coreContractClient.queryContractState()).toEqual('flushing');
        console.log("FSM is in 'Flushing' state as expected.");

        // STEP 3: Finalize the Flush
        // Simulate the off-chain process by updating the AUM oracle with the amount sent to the pump.
        console.log(`Updating AUM oracle with ${toPump.toString()}...`);
        await aumOracleContractClient.updateConfig(account.address, { aum: toPump.toString() }, 'auto');

        // Trigger _process_cache again. The contract should see the AUM updated and return to Idle.
        // We will try `processActiveBatch` again. The batch won't be processed (not enough time has passed,
        // also there are no withdraw requests), but _process_cache runs first and should change the FSM state
        // FLUSHING -> IDLE.
        await coreContractClient.processActiveBatch(account.address, 'auto');
        expect(await coreContractClient.queryContractState()).toEqual('idle');


        // Assert FSM is 'Idle' by successfully executing a state-changing call.
        const finalDepositRes = await coreContractClient.deposit(account.address, { recipient: account.address }, 'auto', "testing idle state", [{ denom: "untrn", amount: "1000" }]);
        expect(finalDepositRes.transactionHash).toBeTruthy();
        console.log("FSM is back to 'Idle', flush is fully finalized.");
    });

    it('should allow deposits and withdrawals while in the FLUSHING state', async () => {
        const { client, coreContractClient, account, coreContractAddress, aumOracleContractClient } = context;

        // Ensure we are starting from a clean 'Idle' state and with some maxBTC to withdraw
        console.log("Depositing funds to prepare for the test...");
        expect(await coreContractClient.queryContractState()).toEqual('idle');
        await aumOracleContractClient.updateConfig(account.address, { aum: "0" }, 'auto');
        await context.liquidationBufferContractClient.updateConfig(account.address, {owned_btc: "0", owned_maxbtc: "0"}, 'auto');
        await coreContractClient.deposit(account.address, { recipient: account.address }, 'auto', "setup deposit", [{ denom: "untrn", amount: "200000" }]);

        const flushPeriod = 10; // As set in instantiation
        console.log(`Waiting for ${flushPeriod + 1} seconds for the deposit to be flushable...`);
        await new Promise(resolve => setTimeout(resolve, (flushPeriod + 1) * 1000));

        // 1. Initiate the flush to move the contract to the 'Flushing' state
        console.log("Executing flushDeposits to enter FLUSHING state...");
        const flushRes = await coreContractClient.flushDeposits(account.address, 'auto');
        await waitForTx(client, flushRes.transactionHash);

        let currentState = await coreContractClient.queryContractState();
        expect(currentState).toEqual('flushing');
        console.log("Contract is now in FLUSHING state.");

        // 2. Perform a deposit and a withdrawal while the contract is flushing
        // These actions should be accepted but processed using cached data.
        const depositAmountDuringFlush = "50000";
        const withdrawAmountDuringFlush = "10000";

        console.log(`Depositing ${depositAmountDuringFlush} untrn while contract is flushing...`);
        const depositWhileFlushingRes = await coreContractClient.deposit(
            account.address,
            { recipient: account.address },
            'auto',
            "deposit during flush",
            [{ denom: "untrn", amount: depositAmountDuringFlush }]
        );
        await waitForTx(client, depositWhileFlushingRes.transactionHash);

        console.log(`Withdrawing ${withdrawAmountDuringFlush} maxBTC while contract is flushing...`);
        const maxBtcDenom = `factory/${coreContractAddress}/maxbtc`;
        const withdrawWhileFlushingRes = await coreContractClient.withdraw(
            account.address,
            'auto',
            "withdraw during flush",
            [{ denom: maxBtcDenom, amount: withdrawAmountDuringFlush }]
        );
        await waitForTx(client, withdrawWhileFlushingRes.transactionHash);

        // 3. Verify the state after these actions
        // The deposit should be held in the core contract, not yet flushed.
        const coreBalance = await client.getBalance(coreContractAddress, 'untrn');
        expect(coreBalance.amount).toEqual(depositAmountDuringFlush);

        // A new withdrawal batch should have been created.
        const activeBatch = await coreContractClient.queryActiveBatch();
        expect(activeBatch).toBeTruthy();
        expect(activeBatch.maxbtc_burned).toEqual(withdrawAmountDuringFlush);
        console.log("Deposit and withdrawal were successfully queued during the flush.");

        // 4. Finalize the original flush to return to 'Idle'
        // We simulate the off-chain part of the flush completing by updating the AUM oracle.
        // For this test, we'll assume the entire initial 200k deposit was sent to the pump for simplicity.
        const flushedAmount = "180000"; // 200k initial deposit minus 10% buffer share minus fees
        console.log(`Simulating flush finalization by updating AUM oracle with ${flushedAmount}...`);
        await aumOracleContractClient.updateConfig(account.address, { aum: flushedAmount }, 'auto');

        // Trigger _process_cache to check the flush status and transition back to Idle
        console.log("Triggering _process_cache to return to IDLE state...");
        await coreContractClient.processCache(account.address, 1.5);

        currentState = await coreContractClient.queryContractState();
        expect(currentState).toEqual('idle');
        console.log("Contract has returned to IDLE state.");

        // 5. Verify the contract is fully operational after returning to Idle
        // For example, the pending deposit is now part of the main balance that can be flushed again.
        console.log("Waiting for another flush period to test the post-flush state...");
        await new Promise(resolve => setTimeout(resolve, (flushPeriod + 1) * 1000));

        const coreBalanceBeforeNextFlush = await client.getBalance(coreContractAddress, 'untrn');
        expect(coreBalanceBeforeNextFlush.amount).toEqual(depositAmountDuringFlush);

        await coreContractClient.flushDeposits(account.address, 'auto');
        const coreBalanceAfterNextFlush = await client.getBalance(coreContractAddress, 'untrn');
        expect(coreBalanceAfterNextFlush.amount).toEqual("0");
        console.log("Successfully flushed the deposit that was made during the previous flush cycle.");

        console.log("Resetting the testing environment to IDLE...");
        await new Promise(resolve => setTimeout(resolve, (flushPeriod + 1) * 1000));
        await aumOracleContractClient.updateConfig(account.address, { aum: (flushedAmount + 36000).toString() }, 'auto');

        // Trigger _process_cache to check the flush status and transition back to Idle
        await coreContractClient.processCache(account.address, 1.5);
        currentState = await coreContractClient.queryContractState();
        expect(currentState).toEqual('idle');
        console.log("Contract has returned to IDLE state.");
    });

    it('should reject new state transitions while not in IDLE state', async () => {
        const { client, coreContractClient, account, collectorContractAddress } = context;

        // 1. Go into WITHDRAWING state
        console.log("Setting up for state transition rejection test...");
        await coreContractClient.deposit(account.address, { recipient: account.address }, 'auto', "setup", [{ denom: "untrn", amount: "50000" }]);
        await coreContractClient.withdraw(account.address, 'auto', "setup", [{ denom: `factory/${context.coreContractAddress}/maxbtc`, amount: "1000" }]);

        const batchActiveDuration = 10;
        console.log(`Waiting ${batchActiveDuration + 1}s for batch to become processable...`);
        await new Promise(resolve => setTimeout(resolve, (batchActiveDuration + 1) * 1000));

        await coreContractClient.processActiveBatch(account.address, 'auto');
        let state = await coreContractClient.queryContractState();
        expect(state).toEqual('withdrawing');
        console.log("Contract is in WITHDRAWING state.");

        // 2. Attempt to call flushDeposits (requires IDLE)
        console.log("Attempting to call flushDeposits from WITHDRAWING state (expected to fail)...");
        await expect(coreContractClient.flushDeposits(account.address, 'auto')).rejects.toThrow(
            /This FSM transition is not allowed/
        );
        console.log("Correctly rejected flushDeposits.");

        // 3. Return to IDLE state
        console.log("Returning to IDLE state to test the next scenario...");
        const withdrawingBatch = await coreContractClient.queryWithdrawingBatch();
        await client.sendTokens(account.address, collectorContractAddress, [{ denom: 'untrn', amount: withdrawingBatch.btc_requested }], 1.5);
        await coreContractClient.processCache(account.address, 1.5);
        state = await coreContractClient.queryContractState();
        expect(state).toEqual('idle');
        console.log("Contract is back in IDLE state.");

        // 4. Go into FLUSHING state
        await coreContractClient.deposit(account.address, { recipient: account.address }, 'auto', "setup", [{ denom: "untrn", amount: "50000" }]);
        const flushPeriod = 10;
        await new Promise(resolve => setTimeout(resolve, (flushPeriod + 1) * 1000));
        await coreContractClient.flushDeposits(account.address, 'auto');
        state = await coreContractClient.queryContractState();
        expect(state).toEqual('flushing');
        console.log("Contract is in FLUSHING state.");

        // 5. Attempt to call processActiveBatch (requires IDLE)
        // First, create a processable batch
        await coreContractClient.withdraw(account.address, 'auto', "setup", [{ denom: `factory/${context.coreContractAddress}/maxbtc`, amount: "1000" }]);
        await new Promise(resolve => setTimeout(resolve, (batchActiveDuration + 1) * 1000));

        console.log("Attempting to call processActiveBatch from FLUSHING state (expected to fail)...");
        await expect(coreContractClient.processActiveBatch(account.address, 'auto')).rejects.toThrow(
            /This FSM transition is not allowed/
        );
        console.log("Correctly rejected processActiveBatch. Test complete.");
    });
});