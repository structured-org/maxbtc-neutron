import { describe, expect, it, beforeAll, afterAll } from 'vitest';
import {
  MaxbtcNeutronCore,
  MaxbtcNeutronFactory,
  MaxbtcNeutronToken,
  MaxbtcNeutronAllowList,
  MaxbtcNeutronExchangeRateProvider,
  MaxbtcNeutronFeeCollector,
  MaxbtcNeutronWaitosaurHolder,
  MaxbtcOracleBinanceAumMock,
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
import { State as FactoryState } from 'maxbtc-neutron-ts-client/lib/contractLib/maxbtcNeutronFactory';

const DEPOSIT_DENOM = 'untrn';

const CoreContractClient = MaxbtcNeutronCore.Client;
const TokenContractClient = MaxbtcNeutronToken.Client;
const FactoryContractClient = MaxbtcNeutronFactory.Client;
const AllowlistContractClient = MaxbtcNeutronAllowList.Client;
const ExchangeRateProviderContractClient =
  MaxbtcNeutronExchangeRateProvider.Client;
const FeeCollectorContractClient = MaxbtcNeutronFeeCollector.Client;
const WaitosaurHolderContractClient = MaxbtcNeutronWaitosaurHolder.Client;

describe('Core', () => {
  const context: {
    park?: Cosmopark;
    wallet?: DirectSecp256k1HdWallet;
    operatorWallet?: DirectSecp256k1HdWallet;
    ceffuBackendWallet?: DirectSecp256k1HdWallet;
    coreContractClient?: InstanceType<typeof CoreContractClient>;
    coreContractOperatorClient?: InstanceType<typeof CoreContractClient>;
    allowlistContractClient?: InstanceType<typeof AllowlistContractClient>;
    exchangeRateProviderContractClient?: InstanceType<
      typeof ExchangeRateProviderContractClient
    >;
    waitosaurHolderContractClient?: InstanceType<
      typeof WaitosaurHolderContractClient
    >;

    account?: AccountData;
    operatorAccount?: AccountData;
    ceffuBackendAccount?: AccountData;
    client?: SigningCosmWasmClient;
    operatorClient?: SigningCosmWasmClient;
    ceffuBackendClient?: SigningCosmWasmClient;
    neutronClient?: InstanceType<typeof NeutronClient>;

    coreContractAddress?: string;
    forwarderContractAddress?: string;
    forwarderLibraryContractAddress?: string;
    waitosaurHolderContractAddress?: string;

    feeCollectorCodeId?: number;
    depositForwarderContractCodeId?: number;
    depositForwarderLibraryContractCodeId?: number;
    exchangeRateProviderContractCodeId?: number;
    allowlistContractCodeId?: number;
    feeCollectorContractCodeId?: number;
    waitosaurHolderContractCodeId?: number;
    coreCodeId?: number;
    tokenCodeId?: number;
    factoryCodeId?: number;
    waitosaurObserverCodeId?: number;

    tokenContractClient?: InstanceType<typeof TokenContractClient>;
    tokenContractAddress?: string;

    feeCollectorContractClient?: InstanceType<
      typeof FeeCollectorContractClient
    >;
    feeCollectorContractAddress?: string;

    factoryContractClient?: InstanceType<typeof FactoryContractClient>;
    factoryContractAddress?: string;

    treasuryAddress?: string;
    waitosaurObserverContractAddress?: string;

    factoryState?: FactoryState;

    binanceAumOracleAddress?: string;
  } = {};

  beforeAll(async (t) => {
    context.park = await setupPark(t, ['neutron']);
    context.wallet = await DirectSecp256k1HdWallet.fromMnemonic(
      context.park.config.wallets.demowallet1.mnemonic,
      {
        prefix: 'neutron',
      },
    );
    context.operatorWallet = await DirectSecp256k1HdWallet.fromMnemonic(
      context.park.config.wallets.demowallet2.mnemonic,
      {
        prefix: 'neutron',
      },
    );
    context.ceffuBackendWallet = await DirectSecp256k1HdWallet.fromMnemonic(
      context.park.config.wallets.demo1.mnemonic,
      {
        prefix: 'neutron',
      },
    );

    context.account = (await context.wallet.getAccounts())[0];
    context.operatorAccount = (await context.operatorWallet.getAccounts())[0];
    context.ceffuBackendAccount = (
      await context.ceffuBackendWallet.getAccounts()
    )[0];

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

    context.operatorClient = await SigningCosmWasmClient.connectWithSigner(
      `http://127.0.0.1:${context.park.ports.neutron.rpc}`,
      context.operatorWallet,
      {
        gasPrice: GasPrice.fromString('0.025untrn'),
      },
    );

    context.ceffuBackendClient = await SigningCosmWasmClient.connectWithSigner(
      `http://127.0.0.1:${context.park.ports.neutron.rpc}`,
      context.ceffuBackendWallet,
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
    it('upload contracts', async () => {
      const { client, account } = context;
      {
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
        context.allowlistContractCodeId = res.codeId;
      }
      {
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
        context.exchangeRateProviderContractCodeId = res.codeId;
      }
      {
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

        context.feeCollectorCodeId = res.codeId;
        context.feeCollectorContractCodeId = res.codeId;
      }
      {
        const res = await client.upload(
          account.address,
          Uint8Array.from(
            fs.readFileSync(
              join(__dirname, '../../../artifacts/maxbtc_neutron_token.wasm'),
            ),
          ),
          1.5,
        );
        expect(res.codeId).toBeGreaterThan(0);
        context.tokenCodeId = res.codeId;
      }
      {
        const res = await client.upload(
          account.address,
          Uint8Array.from(
            fs.readFileSync(
              join(__dirname, '../../../artifacts/maxbtc_neutron_factory.wasm'),
            ),
          ),
          1.5,
        );
        expect(res.codeId).toBeGreaterThan(0);
        context.factoryCodeId = res.codeId;
      }
      {
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
        context.depositForwarderContractCodeId = res.codeId;
      }
      {
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
        context.depositForwarderLibraryContractCodeId = res.codeId;
      }
      {
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
        context.coreCodeId = res.codeId;
      }
      {
        const res = await client.upload(
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
        context.waitosaurHolderContractCodeId = res.codeId;
      }

      {
        const res = await client.upload(
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
        context.waitosaurObserverCodeId = res.codeId;
      }
    });

    it('instantiate oracle binance aum mock', async () => {
      const { client, account } = context;
      const res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../../artifacts/maxbtc_oracle_binance_aum_mock.wasm',
            ),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);

      const instantiateRes =
        await MaxbtcOracleBinanceAumMock.Client.instantiate(
          client,
          account.address,
          res.codeId,
          {},
          'label',
          'auto',
          [],
        );
      expect(instantiateRes.contractAddress).toHaveLength(66);
      context.binanceAumOracleAddress = instantiateRes.contractAddress;
    });

    it('instantiate factory contract', async () => {
      const {
        client,
        account,
        operatorAccount,
        factoryCodeId,
        tokenCodeId,
        coreCodeId,
        waitosaurObserverCodeId,
        feeCollectorCodeId,
        depositForwarderContractCodeId,
        depositForwarderLibraryContractCodeId,
        exchangeRateProviderContractCodeId,
        allowlistContractCodeId,
        waitosaurHolderContractCodeId,
        ceffuBackendAccount,
        binanceAumOracleAddress,
      } = context;

      const instantiateRes = await MaxbtcNeutronFactory.Client.instantiate(
        client,
        account.address,
        factoryCodeId,
        {
          owner: account.address,
          operator: operatorAccount.address,
          ceffu_backend: ceffuBackendAccount.address,
          code_ids: {
            token_code_id: tokenCodeId,
            core_code_id: coreCodeId,
            deposit_forwarder_contract_code_id: depositForwarderContractCodeId,
            deposit_forwarder_library_contract_code_id:
              depositForwarderLibraryContractCodeId,
            exchange_rate_provider_contract_code_id:
              exchangeRateProviderContractCodeId,
            allowlist_contract_code_id: allowlistContractCodeId,
            fee_collector_contract_code_id: feeCollectorCodeId,
            waitosaur_observer_contract_code_id: waitosaurObserverCodeId,
            waitosaur_holder_contract_code_id: waitosaurHolderContractCodeId,
          },
          salt: 'salt',
          deposit_decimals: 6,
          deposit_denom: DEPOSIT_DENOM,
          deposit_cost: '0.01',
          maxbtc_denom: 'maxbtc',
          binance_aum_contract: binanceAumOracleAddress,
          waitosaur_observer_unlocker: account.address,
          fee_collector_params: {
            fee_apy_reduction_percentage: '0.1',
            collection_period_seconds: 10,
          },
          valence_ibc_transfer_params: {
            input_addr: {
              library_account_addr: '',
            },
            output_addr: {
              library_account_addr:
                '0x1234567890123456789012345678901234567890',
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
        },
        'label',
        'auto',
        [],
      );
      expect(instantiateRes.contractAddress).toHaveLength(66);
      context.factoryContractAddress = instantiateRes.contractAddress;

      context.factoryContractClient = new FactoryContractClient(
        client,
        context.factoryContractAddress,
      );
    });

    it('get contracts addresses', async () => {
      const { client, operatorClient, ceffuBackendClient } = context;

      context.factoryState = await context.factoryContractClient.queryState();

      context.allowlistContractClient = new AllowlistContractClient(
        client,
        context.factoryState.allowlist_contract,
      );
      context.exchangeRateProviderContractClient =
        new ExchangeRateProviderContractClient(
          client,
          context.factoryState.exchange_rate_provider_contract,
        );
      context.feeCollectorContractClient = new FeeCollectorContractClient(
        client,
        context.factoryState.fee_collector_contract,
      );
      context.tokenContractClient = new TokenContractClient(
        client,
        context.factoryState.token_contract,
      );
      context.coreContractClient = new CoreContractClient(
        client,
        context.factoryState.core_contract,
      );
      context.coreContractOperatorClient = new CoreContractClient(
        operatorClient,
        context.factoryState.core_contract,
      );
      context.coreContractAddress = context.factoryState.core_contract;
      context.forwarderContractAddress =
        context.factoryState.deposit_forwarder_contract;
      context.forwarderLibraryContractAddress =
        context.factoryState.deposit_forwarder_library_contract;
      context.feeCollectorContractAddress =
        context.factoryState.fee_collector_contract;
      context.tokenContractAddress = context.factoryState.token_contract;

      context.waitosaurObserverContractAddress =
        context.factoryState.waitosaur_observer_contract;
      context.waitosaurHolderContractAddress =
        context.factoryState.waitosaur_holder_contract;
      context.waitosaurHolderContractClient = new WaitosaurHolderContractClient(
        ceffuBackendClient,
        context.waitosaurHolderContractAddress,
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
      const { client, account, tokenContractAddress } = context;
      const balance = await client.getBalance(
        account.address,
        `factory/${tokenContractAddress}/maxbtc`,
      );
      expect(balance.amount).toEqual('198000');
    });

    it('verify deposit balance state', async () => {
      const { coreContractClient } = context;
      const balance = await coreContractClient.queryDepositBalance();
      expect(balance).toEqual('200000');
    });

    describe('run deposit ticks', () => {
      it('try to tick with unauthorized address', async () => {
        const { coreContractClient, account } = context;

        await expect(
          coreContractClient.tick(account.address, 'auto'),
        ).rejects.toThrow(/Unauthorized/);
      });
      it('tick to flush deposits', async () => {
        const { coreContractOperatorClient, operatorAccount } = context;
        const forwarderBalanceBefore = (
          await context.client.getBalance(
            context.forwarderContractAddress,
            DEPOSIT_DENOM,
          )
        ).amount;
        const res = await coreContractOperatorClient.tick(
          operatorAccount.address,
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

        const coreState = await context.coreContractClient.queryContractState();
        expect(coreState).toEqual('deposit_neutron');
      });

      it('unlock waitosaur', async () => {
        const {
          client,
          account,
          waitosaurObserverContractAddress: waitosaurContractAddress,
        } = context;

        // Amount to unlock should be the same as the amount of the deposit and set in the maxbtc-oracle-binance-aum-mock contract

        const result = await client.execute(
          account.address,
          waitosaurContractAddress,
          { unlock: {} },
          {
            amount: coins(5000, 'untrn'),
            gas: '2000000',
          },
        );

        expect(result.transactionHash).toBeTruthy();
      });

      it('run ticks cycle', async () => {
        const { coreContractOperatorClient, operatorAccount } = context;

        let res = await coreContractOperatorClient.tick(
          operatorAccount.address,
          'auto',
        );
        expect(res.transactionHash).toBeTruthy();

        let coreState = await context.coreContractClient.queryContractState();
        expect(coreState).toEqual('deposit_pending');

        res = await coreContractOperatorClient.tick(
          operatorAccount.address,
          'auto',
        );
        expect(res.transactionHash).toBeTruthy();

        coreState = await context.coreContractClient.queryContractState();
        expect(coreState).toEqual('deposit_j_l_p');

        res = await coreContractOperatorClient.tick(
          operatorAccount.address,
          'auto',
        );
        expect(res.transactionHash).toBeTruthy();

        coreState = await context.coreContractClient.queryContractState();
        expect(coreState).toEqual('idle');
      });
    });
  });

  describe('withdrawals', () => {
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
    it('withdraw some amount', async () => {
      const {
        coreContractClient,
        tokenContractAddress,
        client,
        account,
        neutronClient,
      } = context;

      let maxBtcSupply = Number(
        (
          await neutronClient.CosmosBankV1Beta1.query.queryTotalSupply()
        ).data.supply.find((supply) => supply.denom.includes('maxbtc'))
          ?.amount || '0',
      );

      expect(maxBtcSupply).toEqual(394019);

      const depositRes = await coreContractClient.withdraw(
        account.address,
        'auto',
        'withdrawing some wBTC',
        [
          {
            denom: `factory/${tokenContractAddress}/maxbtc`,
            amount: '100000',
          },
        ],
      );
      await waitForTx(client, depositRes.transactionHash);

      maxBtcSupply = Number(
        (
          await neutronClient.CosmosBankV1Beta1.query.queryTotalSupply()
        ).data.supply.find((supply) => supply.denom.includes('maxbtc'))
          ?.amount || '0',
      );

      expect(maxBtcSupply).toEqual(294019);
    });

    it('verify redemption token', async () => {
      const { client, account, tokenContractAddress } = context;
      const balance = await client.getBalance(
        account.address,
        `factory/${tokenContractAddress}/redemption/batch/1`,
      );
      expect(balance.amount).toEqual('100000');
    });

    it('try to tick to re-credit withdrawal from deposit', async () => {
      const {
        coreContractOperatorClient,
        operatorAccount,
        coreContractClient,
      } = context;

      const res = await coreContractOperatorClient.tick(
        operatorAccount.address,
        'auto',
      );
      expect(res.transactionHash).toBeTruthy();

      const state = await coreContractOperatorClient.queryContractState();
      expect(state).toEqual('idle');

      const depositBalance = await coreContractClient.queryDepositBalance();
      expect(depositBalance).toEqual('98483');

      const finalizedBatches = await coreContractClient.queryFinalizedBatches();
      expect(finalizedBatches).toEqual([
        {
          batch_id: 1,
          btc_requested: '101517',
          maxbtc_burned: '100000',
          collected_amount: '101517',
          paid_amount: '0',
          collector_historical_balance: '0',
        },
      ]);
    });
    it('try to tick partially withdraw and go to ticks cycle', async () => {
      const {
        account,
        client,
        coreContractOperatorClient,
        operatorAccount,
        coreContractClient,
        tokenContractAddress,
      } = context;

      const depositRes = await coreContractClient.withdraw(
        account.address,
        'auto',
        'withdrawing some wBTC',
        [
          {
            denom: `factory/${tokenContractAddress}/maxbtc`,
            amount: '120000',
          },
        ],
      );
      await waitForTx(client, depositRes.transactionHash);

      const res = await coreContractOperatorClient.tick(
        operatorAccount.address,
        'auto',
      );
      expect(res.transactionHash).toBeTruthy();

      const state = await coreContractOperatorClient.queryContractState();
      expect(state).toEqual('withdraw_j_l_p');

      const depositBalance = await coreContractClient.queryDepositBalance();
      expect(depositBalance).toEqual('0');

      const withdrawingBatch = await coreContractClient.queryWithdrawingBatch();
      expect(withdrawingBatch).toEqual({
        batch_id: 2,
        btc_requested: '121821',
        maxbtc_burned: '120000',
        collected_amount: '98483',
        paid_amount: '0',
        collector_historical_balance: '0',
      });
    });
    it('tick to withdraw pending', async () => {
      const { coreContractOperatorClient, operatorAccount } = context;

      const res = await coreContractOperatorClient.tick(
        operatorAccount.address,
        'auto',
      );
      expect(res.transactionHash).toBeTruthy();

      const coreState = await context.coreContractClient.queryContractState();
      expect(coreState).toEqual('withdraw_pending');
    });

    it('try to tick to withdraw neutron without ceffu notification, stays in withdraw_pending', async () => {
      const { coreContractOperatorClient, operatorAccount } = context;

      const res = await coreContractOperatorClient.tick(
        operatorAccount.address,
        'auto',
      );
      expect(res.transactionHash).toBeTruthy();

      const coreState = await context.coreContractClient.queryContractState();
      expect(coreState).toEqual('withdraw_pending');
    });

    it('lock waitosaur holder', async () => {
      const { ceffuBackendAccount, waitosaurHolderContractClient } = context;

      const res = await waitosaurHolderContractClient.lock(
        ceffuBackendAccount.address,
        { amount: '50000' },
        'auto',
      );
      expect(res.transactionHash).toBeTruthy();
    });

    it('try to tick without actual means to unlock waitosaur', async () => {
      const { coreContractOperatorClient, operatorAccount } = context;

      await expect(
        coreContractOperatorClient.tick(operatorAccount.address, 'auto'),
      ).rejects.toThrow(/Insufficient asset amount to unlock/);
    });

    it('send wBTC to waitosaur holder to be unlocked', async () => {
      const { account, waitosaurHolderContractAddress, client } = context;

      await client.sendTokens(
        account.address,
        waitosaurHolderContractAddress,
        [{ denom: DEPOSIT_DENOM, amount: '50000' }],
        { amount: [{ denom: 'untrn', amount: '200000' }], gas: '2000000' },
      );
    });

    it('tick to idle', async () => {
      const {
        coreContractClient,
        coreContractOperatorClient,
        operatorAccount,
      } = context;

      let res = await coreContractOperatorClient.tick(
        operatorAccount.address,
        'auto',
      );
      expect(res.transactionHash).toBeTruthy();

      let coreState = await coreContractClient.queryContractState();
      expect(coreState).toEqual('withdraw_neutron');

      const withdrawingBatch = await coreContractClient.queryWithdrawingBatch();
      expect(withdrawingBatch).toEqual({
        batch_id: 2,
        btc_requested: '121821',
        maxbtc_burned: '120000',
        collected_amount: '148483',
        paid_amount: '0',
        collector_historical_balance: '0',
      });

      res = await coreContractOperatorClient.tick(
        operatorAccount.address,
        'auto',
      );
      expect(res.transactionHash).toBeTruthy();

      coreState = await coreContractClient.queryContractState();
      expect(coreState).toEqual('idle');
    });
  });

  describe('Claim withdrawed amount', () => {
    it('should be able to claim tokens from finalized batches', async () => {
      const {
        coreContractClient,
        tokenContractAddress,
        client,
        account,
        operatorAccount,
        neutronClient,
      } = context;

      const operatorAccountBTCBalanceBefore = (
        await neutronClient.CosmosBankV1Beta1.query.queryBalance(
          operatorAccount.address,
          { denom: DEPOSIT_DENOM },
        )
      ).data.balance.amount;

      let claimRes = await coreContractClient.claim(
        account.address,
        { recipient: operatorAccount.address },
        'auto',
        'withdrawing some wBTC',
        [
          {
            denom: `factory/${tokenContractAddress}/redemption/batch/1`,
            amount: '50000',
          },
        ],
      );

      await waitForTx(client, claimRes.transactionHash);

      const operatorAccountBTCBalanceAfter = (
        await neutronClient.CosmosBankV1Beta1.query.queryBalance(
          operatorAccount.address,
          { denom: DEPOSIT_DENOM },
        )
      ).data.balance.amount;

      expect(
        BigInt(operatorAccountBTCBalanceAfter) -
          BigInt(operatorAccountBTCBalanceBefore),
      ).toEqual(BigInt('50758'));

      claimRes = await coreContractClient.claim(
        account.address,
        { recipient: operatorAccount.address },
        'auto',
        'withdrawing some wBTC',
        [
          {
            denom: `factory/${tokenContractAddress}/redemption/batch/2`,
            amount: '50000',
          },
        ],
      );

      await waitForTx(client, claimRes.transactionHash);

      const operatorAccountBTCBalanceAfter2 = (
        await neutronClient.CosmosBankV1Beta1.query.queryBalance(
          operatorAccount.address,
          { denom: DEPOSIT_DENOM },
        )
      ).data.balance.amount;

      expect(
        BigInt(operatorAccountBTCBalanceAfter2) -
          BigInt(operatorAccountBTCBalanceAfter),
      ).toEqual(BigInt('61867'));
    });

    it('try to withdraw from not finalized batch', async () => {
      const {
        coreContractClient,
        account,
        tokenContractAddress,
        operatorAccount,
      } = context;
      const { client } = context;

      const res = await coreContractClient.withdraw(
        account.address,
        'auto',
        'withdrawing some wBTC',
        [
          {
            denom: `factory/${tokenContractAddress}/maxbtc`,
            amount: '1000',
          },
        ],
      );
      await waitForTx(client, res.transactionHash);

      await expect(
        coreContractClient.claim(
          account.address,
          { recipient: operatorAccount.address },
          'auto',
          'withdrawing some wBTC',
          [
            {
              denom: `factory/${tokenContractAddress}/redemption/batch/3`,
              amount: '500',
            },
          ],
        ),
      ).rejects.toThrow(/Batch not in FINALIZED stat/);
    });
  });

  describe('Fee Collector', () => {
    it('should have correct initial config and state', async () => {
      const {
        feeCollectorContractClient,
        account,
        coreContractAddress,
        tokenContractAddress,
      } = context;

      const config = await feeCollectorContractClient.queryConfig();
      const state = await feeCollectorContractClient.queryState();

      expect(config.owner).toEqual(account.address);
      expect(config.core_contract).toEqual(coreContractAddress);
      expect(config.fee_apy_reduction_percentage).toEqual('0.1');
      expect(config.collection_period_seconds).toEqual(10);
      expect(config.fee_denom).toEqual(
        `factory/${tokenContractAddress}/maxbtc`,
      );
      expect(state.last_exchange_rate).toEqual('1'); // Because that was the rate when the contract was instantiated
    });

    it('should successfully collect fees when APY is positive', async () => {
      const {
        feeCollectorContractClient,
        client,
        account,
        coreContractClient,
        tokenContractAddress,
      } = context;

      const maxBtcDenom = `factory/${tokenContractAddress}/maxbtc`;
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
        const {
          feeCollectorContractClient,
          client,
          account,
          tokenContractAddress,
        } = context;
        const maxBtcDenom = `factory/${tokenContractAddress}/maxbtc`;

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
