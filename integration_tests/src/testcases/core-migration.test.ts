import { describe, expect, it, beforeAll, afterAll } from 'vitest';
import {
  MaxbtcNeutronCore,
  MaxbtcNeutronAllowList,
  MaxbtcNeutronExchangeRateProvider,
  MaxbtcNeutronFeeCollector,
} from 'maxbtc-neutron-ts-client';

import { join } from 'path';

import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate';
import { Client as NeutronClient } from '@neutron-org/client-ts';
import {
  AccountData,
  coins,
  DirectSecp256k1HdWallet,
} from '@cosmjs/proto-signing';
import { GasPrice } from '@cosmjs/stargate';
import { setupPark } from '../testSuite';
import fs from 'fs';
import Cosmopark from '@neutron-org/cosmopark';
import { waitForTx } from '../helpers/waitForTx';
import { sleep } from '../helpers/sleep';
import { fromAscii, toAscii } from '@cosmjs/encoding';

const DEPOSIT_DENOM = 'untrn';

const CoreContractClient = MaxbtcNeutronCore.Client;
const AllowlistContractClient = MaxbtcNeutronAllowList.Client;
const ExchangeRateProviderContractClient =
  MaxbtcNeutronExchangeRateProvider.Client;
const FeeCollectorContractClient = MaxbtcNeutronFeeCollector.Client;

describe('Core', () => {
  const context: {
    park?: Cosmopark;
    wallet?: DirectSecp256k1HdWallet;
    coreContractClient?: InstanceType<typeof CoreContractClient>;
    allowlistContractClient?: InstanceType<typeof AllowlistContractClient>;
    exchangeRateProviderContractClient?: InstanceType<
      typeof ExchangeRateProviderContractClient
    >;

    account?: AccountData;
    client?: SigningCosmWasmClient;
    neutronClient?: InstanceType<typeof NeutronClient>;

    coreContractAddress?: string;
    forwarderContractAddress?: string;
    fee_collector_code_id?: number;

    feeCollectorContractClient?: InstanceType<
      typeof FeeCollectorContractClient
    >;
    feeCollectorContractAddress?: string;

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

    context.forwarderContractAddress =
      'neutron1n5ngef5cj32jezpq30ygys3ygggjnduxmlyvrl';
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

    it('instantiate core', async () => {
      const { client, account, fee_collector_code_id } = context;
      const res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../artifacts/migration_contracts/v0.1.0/maxbtc_neutron_core.wasm',
            ),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);
      // Ignore wrong properties errors here, because we are using old version of the contract
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
        account.address,
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
      const addRes = await allowlistContractClient.allow(
        account.address,
        { addresses: [account.address] },
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
        const { account, client, coreContractAddress } = context;

        const res = await client.execute(
          account.address,
          coreContractAddress,
          { flush_deposits: {} },
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
        const { client, coreContractAddress, account } = context;
        const forwarderBalanceBefore = (
          await context.client.getBalance(
            context.forwarderContractAddress,
            DEPOSIT_DENOM,
          )
        ).amount;
        const res = await client.execute(
          account.address,
          coreContractAddress,
          { flush_deposits: {} },
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
        const { account, client, coreContractAddress } = context;
        const res = await client.execute(
          account.address,
          coreContractAddress,
          { flush_deposits: {} },
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

  describe('Migration to new core and mint', () => {
    let totalDeposited: number;
    let lastDepositFlushTime: number;

    beforeAll(async () => {
      const { client, coreContractAddress } = context;

      const totalDepositedStr = await client.queryContractRaw(
        coreContractAddress,
        toAscii('total_deposited'),
      );

      totalDeposited = Number.parseInt(fromAscii(totalDepositedStr), 10);

      const lastDepositFlushTimeStr = await client.queryContractRaw(
        coreContractAddress,
        toAscii('last_deposit_flush_time'),
      );

      lastDepositFlushTime = Number.parseInt(
        fromAscii(lastDepositFlushTimeStr),
        10,
      );
    });
    it('upload contracts nad migrate', async () => {
      const { client, account, coreContractAddress, neutronClient } = context;
      let res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(__dirname, '../../../artifacts/maxbtc_neutron_core.wasm'),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);
      const coreCodeId = res.codeId;

      res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../../artifacts/maxbtc_neutron_waitosaur_holder.wasm',
            ),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);
      const waitosaurHolderCodeId = res.codeId;

      res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../../artifacts/maxbtc_neutron_withdrawal_manager.wasm',
            ),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);
      const withdrawalManagerCodeId = res.codeId;

      res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(__dirname, '../../../artifacts/maxbtc_neutron_token.wasm'),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);
      const tokenCodeId = res.codeId;

      res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../artifacts/contracts_thirdparty/waitasaurus.wasm',
            ),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);
      const waitosaurCodeId = res.codeId;

      await client.sendTokens(
        account.address,
        coreContractAddress,
        [{ denom: DEPOSIT_DENOM, amount: '100000' }],
        { amount: [{ denom: 'untrn', amount: '200000' }], gas: '2000000' },
      );

      const coreTokenBalanceBeforeMigration = (
        await neutronClient.CosmosBankV1Beta1.query.queryBalance(
          coreContractAddress,
          { denom: DEPOSIT_DENOM },
        )
      ).data.balance.amount;

      const fee = {
        amount: coins(5000, 'untrn'),
        gas: '2000000',
      };

      const result = await client.migrate(
        account.address,
        coreContractAddress,
        tokenCodeId,
        {
          waitosaur_observer_code_id: waitosaurCodeId,

          waitosaur_holder_code_id: waitosaurHolderCodeId,
          core_code_id: coreCodeId,
          withdrawal_manager_code_id: withdrawalManagerCodeId,
          factory_contract:
            'neutron1nxshmmwrvxa2cp80nwvf03t8u5kvl2ttr8m8f43vamudsqrdvs8qqvfwpj',
          operator:
            'neutron1nxshmmwrvxa2cp80nwvf03t8u5kvl2ttr8m8f43vamudsqrdvs8qqvfwpj',
          waitosaur_observer_unlocker: account.address,
          binance_aum_contract:
            'neutron1nxshmmwrvxa2cp80nwvf03t8u5kvl2ttr8m8f43vamudsqrdvs8qqvfwpj',
          ceffu_backend:
            'neutron1nxshmmwrvxa2cp80nwvf03t8u5kvl2ttr8m8f43vamudsqrdvs8qqvfwpj',
          salt: 'salt',
          exchange_rate_stale_period: '60',
          withdrawal_cost: '0.01',
        },
        fee,
      );

      const tx = await context.client.getTx(result.transactionHash);
      const { events } = tx;
      const newCoreContractAddress = events.find((e) => e.type === 'wasm')
        .attributes[1].value;

      const totalDepositedStr = await client.queryContractRaw(
        coreContractAddress,
        toAscii('total_deposited'),
      );

      const totalDepositedMigrated = Number.parseInt(
        fromAscii(totalDepositedStr),
        10,
      );
      expect(totalDepositedMigrated).toEqual(totalDeposited);

      const lastDepositFlushTimeStr = await client.queryContractRaw(
        coreContractAddress,
        toAscii('last_deposit_flush_time'),
      );

      const lastDepositFlushTimeMigrated = Number.parseInt(
        fromAscii(lastDepositFlushTimeStr),
        10,
      );

      expect(lastDepositFlushTimeMigrated).toEqual(lastDepositFlushTime);

      const coreTokenBalanceAfterMigration = (
        await neutronClient.CosmosBankV1Beta1.query.queryBalance(
          coreContractAddress,
          { denom: DEPOSIT_DENOM },
        )
      ).data.balance.amount;

      expect(coreTokenBalanceAfterMigration).toEqual('0');

      const newCoreBalanceAfterMigration = (
        await neutronClient.CosmosBankV1Beta1.query.queryBalance(
          newCoreContractAddress,
          { denom: DEPOSIT_DENOM },
        )
      ).data.balance.amount;

      expect(newCoreBalanceAfterMigration).toEqual(
        coreTokenBalanceBeforeMigration,
      );
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
