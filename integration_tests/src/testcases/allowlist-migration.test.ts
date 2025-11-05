import { describe, expect, it, beforeAll, afterAll } from 'vitest';
import { MaxbtcNeutronAllowList } from 'maxbtc-neutron-ts-client';

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
import { fromAscii, toAscii } from '@cosmjs/encoding';

const AllowlistContractClient = MaxbtcNeutronAllowList.Client;

describe('Core', () => {
  const context: {
    park?: Cosmopark;
    wallet?: DirectSecp256k1HdWallet;

    account?: AccountData;
    client?: SigningCosmWasmClient;
    neutronClient?: InstanceType<typeof NeutronClient>;

    allowlistContractClient?: InstanceType<typeof AllowlistContractClient>;
    allowlistContractAddress?: string;
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
    it('instantiate allowlist', async () => {
      const { client, account } = context;
      const res = await client.upload(
        account.address,
        Uint8Array.from(
          fs.readFileSync(
            join(
              __dirname,
              '../../artifacts/migration_contracts/v0.1.0/maxbtc_neutron_allow_list.wasm',
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
        account.address,
      );
      expect(instantiateRes.contractAddress).toHaveLength(66);
      context.allowlistContractClient = new MaxbtcNeutronAllowList.Client(
        client,
        instantiateRes.contractAddress,
      );

      context.allowlistContractAddress = instantiateRes.contractAddress;
    });
  });

  describe('Migration to new allow list and and set zkMe configuration', () => {
    it('upload contracts nad migrate', async () => {
      const { client, account, allowlistContractAddress } = context;

      {
        const zkMeSettingsRaw = await client.queryContractRaw(
          allowlistContractAddress,
          toAscii('zk_me_settings'),
        );

        expect(zkMeSettingsRaw.length).toBe(0);
      }

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
      const allowlistCodeId = res.codeId;

      const fee = {
        amount: coins(5000, 'untrn'),
        gas: '2000000',
      };

      await client.migrate(
        account.address,
        allowlistContractAddress,
        allowlistCodeId,
        {},
        fee,
      );

      await client.execute(
        account.address,
        allowlistContractAddress,
        {
          update_zk_me_settings: {
            settings: {
              contract:
                'neutron19t7s6aa9289e563mu9qrx5nh80xtn4vr5afdu8yctej6f7w6k9usv87acp',
              cooperator: 'neutron13h2r2k8jwd0utnrzfud3n8uxq33lshvhql9yvv',
            },
          },
        },
        {
          amount: coins(5000, 'untrn'),
          gas: '2000000',
        },
      );

      {
        const zkMeSettingsRaw = await client.queryContractRaw(
          allowlistContractAddress,
          toAscii('zk_me_settings'),
        );

        const zkMeSettings = JSON.parse(fromAscii(zkMeSettingsRaw));

        expect(zkMeSettings).toEqual({
          contract:
            'neutron19t7s6aa9289e563mu9qrx5nh80xtn4vr5afdu8yctej6f7w6k9usv87acp',
          cooperator: 'neutron13h2r2k8jwd0utnrzfud3n8uxq33lshvhql9yvv',
        });
      }
    });
  });
});
