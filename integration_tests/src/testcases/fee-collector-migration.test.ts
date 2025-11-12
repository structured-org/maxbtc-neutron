import { describe, expect, it, beforeAll, afterAll } from 'vitest';
import {
  MaxbtcNeutronFeeCollector,
  MaxbtcNeutronExchangeRateProvider,
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

const FeeCollectorContractClient = MaxbtcNeutronFeeCollector.Client;
const ExchangeRateProviderContractClient =
  MaxbtcNeutronExchangeRateProvider.Client;

describe('Core', () => {
  const context: {
    park?: Cosmopark;
    wallet?: DirectSecp256k1HdWallet;

    account?: AccountData;
    client?: SigningCosmWasmClient;
    neutronClient?: InstanceType<typeof NeutronClient>;

    feeCollectorContractClient?: InstanceType<
      typeof FeeCollectorContractClient
    >;
    feeCollectorContractAddress?: string;

    exchangeRateProviderContractClient?: InstanceType<
      typeof ExchangeRateProviderContractClient
    >;
    exchangeRateProviderContractAddress?: string;
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
  });

  afterAll(async () => {
    await context.park.stop();
  });

  describe('upload and instantiate contracts', () => {
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
      const instantiateRes =
        await ExchangeRateProviderContractClient.instantiate(
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
        new ExchangeRateProviderContractClient(
          client,
          instantiateRes.contractAddress,
        );

      context.exchangeRateProviderContractAddress =
        instantiateRes.contractAddress;
    });

    it('instantiate fee collector', async () => {
      const { client, account, exchangeRateProviderContractAddress } = context;
      const res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../artifacts/migration_contracts/v0.1.0/maxbtc_neutron_fee_collector.wasm',
            ),
          ),
        ),
        1.5,
      );
      expect(res.codeId).toBeGreaterThan(0);
      const instantiateRes = await FeeCollectorContractClient.instantiate(
        client,
        account.address,
        res.codeId,
        {
          owner: account.address,
          collection_period_seconds: 100,
          core_contract: exchangeRateProviderContractAddress,
          fee_apy_reduction_percentage: '0.1',
          fee_denom: 'maxBTC',
          maxbtc_decimals: 6,
        },
        'label',
        'auto',
        [],
        account.address,
      );
      expect(instantiateRes.contractAddress).toHaveLength(66);
      context.feeCollectorContractClient = new FeeCollectorContractClient(
        client,
        instantiateRes.contractAddress,
      );

      context.feeCollectorContractAddress = instantiateRes.contractAddress;
    });
  });

  describe('Migration to new fee collector with changed owner code', () => {
    it('upload contracts nad migrate', async () => {
      const {
        client,
        account,
        feeCollectorContractAddress,
        feeCollectorContractClient,
        exchangeRateProviderContractAddress,
      } = context;

      {
        const config = await feeCollectorContractClient.queryConfig();

        expect(config).toEqual({
          owner: account.address,
          collection_period_seconds: 100,
          core_contract: exchangeRateProviderContractAddress,
          fee_apy_reduction_percentage: '0.1',
          fee_denom: 'maxBTC',
          maxbtc_decimals: 6,
        });
      }

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
      const feeCollectorCodeId = res.codeId;

      const fee = {
        amount: coins(5000, 'untrn'),
        gas: '2000000',
      };

      await client.migrate(
        account.address,
        feeCollectorContractAddress,
        feeCollectorCodeId,
        {},
        fee,
      );

      {
        const config = await feeCollectorContractClient.queryConfig();

        expect(config).toEqual({
          collection_period_seconds: 100,
          core_contract: exchangeRateProviderContractAddress,
          fee_apy_reduction_percentage: '0.1',
          fee_denom: 'maxBTC',
          maxbtc_decimals: 6,
        });

        const owner = await feeCollectorContractClient.queryOwnership();

        expect(owner).toEqual({
          owner: account.address,
          pending_expiry: null,
          pending_owner: null,
        });
      }
    });
  });
});
