import { describe, expect, it, beforeAll, afterAll } from 'vitest';
import {
  MaxbtcNeutronCore,
  MaxbtcNeutronAllowList,
  MaxbtcNeutronExchangeRateProvider, MaxbtcNeutronFeeCollector, MaxbtcNeutronApyCalculator,
} from 'maxbtc-neutron-ts-client';

import { join } from 'path';

import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate';
import { Client as NeutronClient } from '@neutron-org/client-ts';
import { AccountData, DirectSecp256k1HdWallet } from '@cosmjs/proto-signing';
import { GasPrice } from '@cosmjs/stargate';
import { setupPark } from '../testSuite';
import fs from 'fs';
import Cosmopark from '@neutron-org/cosmopark';
import { waitForTx } from '../helpers/waitForTx';
import { sleep } from '../helpers/sleep';

const DEPOSIT_DENOM = 'untrn';
const APY_CALC_PERIOD_TIMEOUT = 10; // seconds
const APY_CALC_PERIODS_TO_KEEP = 24;

const CoreContractClient = MaxbtcNeutronCore.Client;
const AllowlistContractClient = MaxbtcNeutronAllowList.Client;
const ExchangeRateProviderContractClient =
  MaxbtcNeutronExchangeRateProvider.Client;
const FeeCollectorContractClient = MaxbtcNeutronFeeCollector.Client;
const ApyCalculatorContractClient = MaxbtcNeutronApyCalculator.Client;

describe('Core', () => {
  const context: {
    park?: Cosmopark;
    wallet?: DirectSecp256k1HdWallet;
    coreContractClient?: InstanceType<typeof CoreContractClient>;
    allowlistContractClient?: InstanceType<typeof AllowlistContractClient>;
    exchangeRateProviderContractClient?: InstanceType<
      typeof ExchangeRateProviderContractClient
    >;
    apyCalculatorContractClient?: InstanceType<typeof ApyCalculatorContractClient>;

    account?: AccountData;
    client?: SigningCosmWasmClient;
    neutronClient?: InstanceType<typeof NeutronClient>;

    coreContractAddress?: string;
    forwarderContractAddress?: string;
    forwarderLibraryContractAddress?: string;
    fee_collector_code_id?: number;

    feeCollectorContractClient?: InstanceType<typeof FeeCollectorContractClient>;
    feeCollectorContractAddress?: string;
    apyCalculatorContractAddress?: string;

    treasuryAddress?: string;
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
    context.treasuryAddress =
      'neutron1nxshmmwrvxa2cp80nwvf03t8u5kvl2ttr8m8f43vamudsqrdvs8qqvfwpj';
  });

  afterAll(async () => {
    await context.park.stop();
  });

  describe('upload and instantiate contracts', () => {
    it('instantiate allowlist', async () => {
      const { client, account } = context;
      const res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../../artifacts/maxbtc_neutron_allow_list.wasm',
            ),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);
      const instantiateRes = await MaxbtcNeutronAllowList.Client.instantiate(
        client,
        account.address,
        res.codeId,
        { owner: account.address },
        'label',
        'auto',
        [],
      );
      expect(instantiateRes.contractAddress).toHaveLength(66);
      context.allowlistContractClient = new MaxbtcNeutronAllowList.Client(
        client,
        instantiateRes.contractAddress,
      );
    });
    it('instantiate exchange rate provider', async () => {
      const { client, account } = context;
      const res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../../artifacts/maxbtc_neutron_exchange_rate_provider.wasm',
            ),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);
      const instantiateRes = await MaxbtcNeutronAllowList.Client.instantiate(
        client,
        account.address,
        res.codeId,
        { owner: account.address },
        'label',
        'auto',
        [],
      );
      expect(instantiateRes.contractAddress).toHaveLength(66);
      context.exchangeRateProviderContractClient =
        new MaxbtcNeutronExchangeRateProvider.Client(
          client,
          instantiateRes.contractAddress,
        );
    });

    it('upload fee collector', async () => {
      const { client, account } = context;
      const res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../../artifacts/maxbtc_neutron_fee_collector.wasm',
            ),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);

      context.fee_collector_code_id = res.codeId;
    });

    it('instantiate forwarder (valence base account)', async () => {
      const { client, account } = context;
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
        approved_libraries: [],
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
      context.forwarderContractAddress = instantiateRes.contractAddress;
    });

    it('instantiate forwarder library (valence ibc transfer library)', async () => {
      const { client, account } = context;
      const res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../../artifacts/valence_neutron_ibc_transfer_library.wasm',
            ),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);

      const instantiateMsg = {
        owner: account.address,
        processor: account.address,
        config: {
          input_addr: { library_account_addr: context.forwarderContractAddress },
          output_addr: {
            library_account_addr: '0x1234567890123456789012345678901234567890',
          },
          denom: {
            native:
              'ibc/0E293A7622DC9A6439DB60E6D234B5AF446962E27CA3AB44D0590603DFF6968E',
          },
          amount: 'full_amount',
          memo: '',
          remote_chain_info: {
            channel_id: 'channel-1',
          },
          denom_to_pfm_map: {},
          eureka_config: {
            callback_contract:
              'cosmos1lqu9662kd4my6dww4gzp3730vew0gkwe0nl9ztjh0n5da0a8zc4swsvd22',
            action_contract:
              'cosmos1clswlqlfm8gpn7n5wu0ypu0ugaj36urlhj7yz30hn7v7mkcm2tuqy9f8s5',
            recover_address: 'cosmos1ep2umj6kn34g2ttjalsc5r9w8pt7sv4x9z0q26',
            source_channel: '08-wasm-1369',
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
      context.forwarderLibraryContractAddress = instantiateRes.contractAddress;

      const approveRes = await client.execute(
        account.address,
        context.forwarderContractAddress,
        {
          approve_library: {
            library: context.forwarderLibraryContractAddress,
          },
        },
        'auto',
      );
      expect(approveRes.transactionHash).toHaveLength(64);
      await waitForTx(client, approveRes.transactionHash);
    });

    it('instantiate core', async () => {
      const { client, account, fee_collector_code_id } = context;
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
          deposit_forwarder_contract: context.forwarderContractAddress,
          deposit_decimals: 6,
          deposit_denom: DEPOSIT_DENOM,
          deposit_cost: '0.01',
          deposit_flush_period: 60,
          maxbtc_denom: 'maxbtc',
          owner: account.address,
          exchange_rate_provider_contract:
            context.exchangeRateProviderContractClient.contractAddress,
          allowlist_contract: context.allowlistContractClient.contractAddress,
          fee_collector_params: {
            code_id: fee_collector_code_id,
            collection_period_seconds: 10,
            fee_apy_reduction_percentage: '0.1',
            salt: 'Z3Rmbw==',
          },
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
    it('get fee collector address and instantiate client', async () => {
      const { client, coreContractClient } = context;
      // The core contract stores the address of the fee collector it created.
      const coreConfig = await coreContractClient.queryConfig();
      const feeCollectorAddress = coreConfig.fee_collector_contract;

      expect(feeCollectorAddress).toBeTruthy();
      expect(feeCollectorAddress).toHaveLength(66);

      context.feeCollectorContractAddress = feeCollectorAddress;
      context.feeCollectorContractClient = new FeeCollectorContractClient(
          client,
          feeCollectorAddress,
      );
    });

    it('instantiate apy calculator', async () => {
      const { client, account, coreContractAddress } = context;
      const res = await client.upload(
          account.address,
          Uint8Array.from(
              fs.readFileSync(
                  join(
                      __dirname,
                      '../../../artifacts/maxbtc_neutron_apy_calculator.wasm',
                  ),
              ),
          ),
          1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);
      const instantiateRes = await ApyCalculatorContractClient.instantiate(
          client,
          account.address,
          res.codeId,
          {
            owner: account.address,
            core_contract: coreContractAddress,
            period_timeout: APY_CALC_PERIOD_TIMEOUT,
            periods_to_keep: APY_CALC_PERIODS_TO_KEEP,
          },
          'apy_calculator',
          'auto',
          [],
      );
      expect(instantiateRes.contractAddress).toHaveLength(66);
      context.apyCalculatorContractAddress = instantiateRes.contractAddress;
      context.apyCalculatorContractClient = new ApyCalculatorContractClient(
          client,
          instantiateRes.contractAddress,
      );
    });
  });

  describe('deposits', () => {
    it('should not be able to deposit from a not allowed address', async () => {
      const { coreContractClient, account } = context;
      await expect(
        coreContractClient.deposit(
          account.address,
          { recipient: account.address },
          'auto',
          'seeding deposit for cycle test',
          [{ denom: 'untrn', amount: '200000' }],
        ),
      ).rejects.toThrow(/Recipient address not allowed to mint maxBTC/);
    });

    it('update allow list', async () => {
      const { allowlistContractClient, client, account } = context;

      // Add the account to the allowlist
      const addRes = await allowlistContractClient.updateAllowList(
        account.address,
        { allow_list: [account.address] },
        'auto',
        'adding account to allowlist',
      );
      expect(addRes.transactionHash).toBeTruthy();
      await waitForTx(client, addRes.transactionHash);
      // Verify the account is now allowed
      const isAllowed = await allowlistContractClient.queryIsAddressAllowed({
        address: account.address,
      });
      expect(isAllowed).toBeTruthy();
    });

    it('normal deposit', async () => {
      const { coreContractClient, client, account } = context;
      const depositRes = await coreContractClient.deposit(
        account.address,
        { recipient: account.address },
        'auto',
        'seeding deposit for cycle test',
        [{ denom: 'untrn', amount: '200000' }],
      );
      await waitForTx(client, depositRes.transactionHash);
      await updateExchangeRate(
        context.account.address,
        context.neutronClient,
        context.coreContractClient,
        context.exchangeRateProviderContractClient,
        context.forwarderContractAddress,
      );
    });

    it('verify deposit', async () => {
      const { client, account, coreContractAddress } = context;
      const balance = await client.getBalance(
        account.address,
        `factory/${coreContractAddress}/maxbtc`,
      );
      expect(balance.amount).toEqual('198000');
    });

    describe('flush deposits', () => {
      it('try to flush deposits before flush period', async () => {
        const { coreContractClient, account } = context;
        const res = await coreContractClient.flushDeposits(
          account.address,
          'auto',
        );
        expect(res.transactionHash).toBeTruthy();
        const tx = await context.client.getTx(res.transactionHash);
        const { events } = tx;
        const wasmEvent = events.find(
          (e) =>
            e.type === 'wasm' &&
            e.attributes.some(
              (attr) =>
                attr.key === 'action' && attr.value === 'flush_deposits',
            ),
        );
        expect(
          wasmEvent.attributes.find(
            (a) => a.key === 'status' && a.value === 'not_enough_time_elapsed',
          ),
        ).toBeTruthy();
      });
      it('update config', async () => {
        const { coreContractClient, account } = context;
        const res = await coreContractClient.updateConfig(
          account.address,
          {
            deposit_flush_period: 10,
          } as any,
          'auto',
        );
        expect(res.transactionHash).toBeTruthy();
        await waitForTx(context.client, res.transactionHash);
      });
      it('flush deposits before flush period', async () => {
        const { coreContractClient, account } = context;
        const forwarderBalanceBefore = (
          await context.client.getBalance(
            context.forwarderContractAddress,
            DEPOSIT_DENOM,
          )
        ).amount;
        const res = await coreContractClient.flushDeposits(
          account.address,
          'auto',
        );
        expect(res.transactionHash).toBeTruthy();
        const tx = await context.client.getTx(res.transactionHash);
        const { events } = tx;
        const wasmEvent = events.find(
          (e) =>
            e.type === 'wasm' &&
            e.attributes.some(
              (attr) =>
                attr.key === 'action' && attr.value === 'flush_deposits',
            ),
        );
        expect(
          wasmEvent.attributes.find(
            (a) => a.key === 'flushed' && Number(a.value) > 0,
          ),
        ).toBeTruthy();
        await waitForTx(context.client, res.transactionHash);
        const forwarderBalanceAfter = (
          await context.client.getBalance(
            context.forwarderContractAddress,
            DEPOSIT_DENOM,
          )
        ).amount;
        expect(BigInt(forwarderBalanceAfter)).toBeGreaterThan(
          BigInt(forwarderBalanceBefore),
        );
      });
      it('no flush deposits right after the flush', async () => {
        const { coreContractClient, account } = context;
        const res = await coreContractClient.flushDeposits(
          account.address,
          'auto',
        );
        expect(res.transactionHash).toBeTruthy();
        const tx = await context.client.getTx(res.transactionHash);
        const { events } = tx;
        const wasmEvent = events.find(
          (e) =>
            e.type === 'wasm' &&
            e.attributes.some(
              (attr) =>
                attr.key === 'action' && attr.value === 'flush_deposits',
            ),
        );
        expect(
          wasmEvent.attributes.find(
            (a) => a.key === 'status' && a.value === 'not_enough_time_elapsed',
          ),
        ).toBeTruthy();
      });
    });
  });

  describe('Fee Collector', () => {
    it('should have correct initial config and state', async () => {
      const {
        feeCollectorContractClient,
        account,
        coreContractAddress,
      } = context;

      const config = await feeCollectorContractClient.queryConfig();
      const state = await feeCollectorContractClient.queryState();

      expect(config.owner).toEqual(account.address);
      expect(config.core_contract).toEqual(coreContractAddress);
      expect(config.fee_apy_reduction_percentage).toEqual('0.1');
      expect(config.collection_period_seconds).toEqual(10);
      expect(config.fee_denom).toEqual(
          `factory/${coreContractAddress}/maxbtc`,
      );
      expect(state.last_exchange_rate).toEqual("1"); // Because that was the rate when the contract was instantiated
    });

    it('should successfully collect fees when APY is positive', async () => {
      const {
        feeCollectorContractClient,
        client,
        account,
        coreContractClient,
        coreContractAddress
      } = context;

      const maxBtcDenom = `factory/${coreContractAddress}/maxbtc`;
      const balanceBefore = await client.getBalance(
          feeCollectorContractClient.contractAddress,
          maxBtcDenom,
      );
      const stateBefore = await feeCollectorContractClient.queryState();

      // The previous deposit test already caused a rate increase, so APY is positive.
      // We just need to wait for the collection period again.
      await sleep(1500);

      // Collect the fee
      const collectRes = await feeCollectorContractClient.collectFee(
          account.address,
          'auto',
      );
      await waitForTx(client, collectRes.transactionHash);

      // Verify fee was collected by checking the balance
      const balanceAfter = await client.getBalance(
          feeCollectorContractClient.contractAddress,
          maxBtcDenom,
      );
      expect(BigInt(balanceAfter.amount)).toBeGreaterThan(
          BigInt(balanceBefore.amount),
      );

      // Verify the contract's internal state was updated via the reply handler
      const stateAfter = await feeCollectorContractClient.queryState();
      const newRate = await coreContractClient.queryExchangeRate();
      expect(stateAfter.last_exchange_rate).toEqual(newRate);
      expect(Number(stateAfter.last_collection_timestamp)).toBeGreaterThan(
          Number(stateBefore.last_collection_timestamp),
      );
    });

    it('should fail to collect fee before collection period ends', async () => {
      const { feeCollectorContractClient, account } = context;
      await expect(
          feeCollectorContractClient.collectFee(account.address, 'auto'),
      ).rejects.toThrow(/Fee collection is not allowed yet/);
    });

    it('should fail to collect fee with negative or zero APY', async () => {
      const { feeCollectorContractClient, account } = context;
      // Wait for the short collection period to pass
      await sleep(11000);

      // The exchange rate hasn't changed since instantiation, so APY is zero.
      await expect(
          feeCollectorContractClient.collectFee(account.address, 'auto'),
      ).rejects.toThrow(/APY is not positive/);
    });

    describe('Claiming & Config', () => {
      it('should allow owner to claim collected fees', async () => {
        const { feeCollectorContractClient, client, account, coreContractAddress } =
            context;
        const maxBtcDenom = `factory/${coreContractAddress}/maxbtc`;

        const feeCollectorBalance = await client.getBalance(
            feeCollectorContractClient.contractAddress,
            maxBtcDenom,
        );
        // Ensure there's a balance to claim from the previous test
        expect(BigInt(feeCollectorBalance.amount)).toBeGreaterThan(0n);

        const recipientBalanceBefore = await client.getBalance(
            account.address,
            maxBtcDenom,
        );
        const claimAmount = { denom: maxBtcDenom, amount: '10' };

        await feeCollectorContractClient.claim(
            account.address,
            { amount: claimAmount, recipient: account.address },
            'auto',
        );

        const recipientBalanceAfter = await client.getBalance(
            account.address,
            maxBtcDenom,
        );

        const expectedBalance =
            BigInt(recipientBalanceBefore.amount) + BigInt(claimAmount.amount);
        expect(BigInt(recipientBalanceAfter.amount)).toEqual(expectedBalance);
      });

      it('should allow owner to update the configuration', async () => {
        const { feeCollectorContractClient, account, client } = context;
        const newPeriodHours = 2;
        const newPercentage = '0.25';

        const updateRes = await feeCollectorContractClient.updateConfig(
            account.address,
            {
              collection_period_hours: newPeriodHours,
              fee_apy_reduction_percentage: newPercentage,
            },
            'auto',
        );
        await waitForTx(client, updateRes.transactionHash);

        const newConfig = await feeCollectorContractClient.queryConfig();
        expect(newConfig.collection_period_seconds).toEqual(
            newPeriodHours * 3600,
        );
        expect(newConfig.fee_apy_reduction_percentage).toEqual(newPercentage);
      });
    });
  });

  describe('Apy Calculator', () => {
    it('should have the correct initial configuration', async () => {
      const { apyCalculatorContractClient, account, coreContractAddress } =
          context;

      const config = await apyCalculatorContractClient.queryGetConfig();
      expect(config.core_contract).toEqual(coreContractAddress);
      expect(config.period_timeout).toEqual(APY_CALC_PERIOD_TIMEOUT);
      expect(config.periods_to_keep).toEqual(APY_CALC_PERIODS_TO_KEEP);

      const ownership = await apyCalculatorContractClient.queryOwnership();
      expect(ownership.owner).toEqual(account.address);
    });

    it('should update the exchange rate snapshot for the first time', async () => {
      const { apyCalculatorContractClient, account } = context;
      // The `deposits` tests have already run and updated the rate in the provider.
      // This first snapshot will capture that updated rate.
      // The first call is allowed irrespective of time, as last_update is 0.
      const res = await apyCalculatorContractClient.updateExchangeRates(
          account.address,
          'auto',
      );
      expect(res.transactionHash).toHaveLength(64);
      await waitForTx(context.client, res.transactionHash);
    });

    it('should fail to update exchange rate snapshot before the timeout period', async () => {
      const { apyCalculatorContractClient, account } = context;
      await expect(
          apyCalculatorContractClient.updateExchangeRates(account.address, 'auto'),
      ).rejects.toThrow(/Too early/);
    });

    it('should fail to query APY with only one data point', async () => {
      const { apyCalculatorContractClient } = context;
      // With only one snapshot, start time and end time are the same,
      // leading to an error.
      await expect(
          apyCalculatorContractClient.queryGetApy({ time_span_hours: 1 }),
      ).rejects.toThrow(/Period end is before period start/);
    });

    it('should successfully update the exchange rate snapshot a second time', async () => {
      const { apyCalculatorContractClient, account, coreContractClient } = context;
      // Wait for the timeout period to pass
      await sleep((APY_CALC_PERIOD_TIMEOUT + 1) * 1000);

      // Make another deposit to change the AUM, which will change the exchange rate
      const depositRes = await coreContractClient.deposit(
          account.address,
          { recipient: account.address },
          'auto',
          'second deposit for apy test',
          [{ denom: 'untrn', amount: '100000' }],
      );
      await waitForTx(context.client, depositRes.transactionHash);

      // Update the exchange rate provider with the new AUM
      await updateExchangeRate(
          context.account.address,
          context.neutronClient,
          context.coreContractClient,
          context.exchangeRateProviderContractClient,
          context.forwarderContractAddress,
      );

      // Trigger the snapshot in the APY calculator
      const res = await apyCalculatorContractClient.updateExchangeRates(
          account.address,
          'auto',
      );
      expect(res.transactionHash).toHaveLength(64);
      await waitForTx(context.client, res.transactionHash);
    });

    it('should calculate the APY based on the two snapshots', async () => {
      const { apyCalculatorContractClient } = context;

      const res = await apyCalculatorContractClient.queryGetApy({
        time_span_hours: 1,
      });

      const startRate = Number(res.start_exchange_rate.exchange_rate);
      const endRate = Number(res.end_exchange_rate.exchange_rate);
      const startTs = Number(res.start_exchange_rate.timestamp);
      const endTs = Number(res.end_exchange_rate.timestamp);


      // Because we made a deposit, the rate should have increased.
      expect(endRate).toBeGreaterThan(startRate);
      expect(endTs).toBeGreaterThan(startTs);
      expect(Number(res.apy)).toBeGreaterThan(0);
    });

    it('should fail to query with an out of range timespan', async () => {
      const { apyCalculatorContractClient } = context;

      await expect(
          apyCalculatorContractClient.queryGetApy({
            time_span_hours: APY_CALC_PERIODS_TO_KEEP,
          }),
      ).rejects.toThrow(/Timespan is out of range/);
    });
  });
});

const updateExchangeRate = async (
  account: string,
  neutronClient: InstanceType<typeof NeutronClient>,
  coreClient: InstanceType<typeof CoreContractClient>,
  exchangeRateProviderClient: InstanceType<
    typeof ExchangeRateProviderContractClient
  >,
  forwarderAddress: string,
) => {
  const maxBtcSupply = Number(
    (
      await neutronClient.CosmosBankV1Beta1.query.queryTotalSupply()
    ).data.supply.find((supply) => supply.denom.includes('maxbtc'))?.amount ||
      '0',
  );
  const coreBTCBalance = (
    await neutronClient.CosmosBankV1Beta1.query.queryBalance(
      coreClient.contractAddress,
      { denom: DEPOSIT_DENOM },
    )
  ).data.balance.amount;

  const forwarderBTCBalance = (
    await neutronClient.CosmosBankV1Beta1.query.queryBalance(forwarderAddress, {
      denom: DEPOSIT_DENOM,
    })
  ).data.balance.amount;

  const aum = Number(coreBTCBalance) + Number(forwarderBTCBalance);

  await exchangeRateProviderClient.updateExchangeRate(account, {
    rate: (aum / maxBtcSupply).toString(),
  });

  await sleep(3000);
};
