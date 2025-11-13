import { describe, expect, it, beforeAll, afterAll } from 'vitest';
import { MaxbtcNeutronCoinfactoryWrapper } from 'maxbtc-neutron-ts-client';
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate';
import { Client as NeutronClient } from '@neutron-org/client-ts';
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing';
import Cosmopark from '@neutron-org/cosmopark';
import { setupPark } from '../testSuite';
import { join } from 'path';
import fs from 'fs';
import { GasPrice } from '@cosmjs/stargate';

const DEPOSIT_DENOM = 'untrn';
const CoinfactoryWrapper = MaxbtcNeutronCoinfactoryWrapper.Client;

describe('Core', () => {
  const context: {
    park?: Cosmopark;
    wallet?: DirectSecp256k1HdWallet;
    coinfactoryWrapper?: InstanceType<typeof CoinfactoryWrapper>;
    coinfactoryDenom?: string;
    account?: { address: string };
    client?: SigningCosmWasmClient;
    neutronClient?: InstanceType<typeof NeutronClient>;
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
    context.client = await SigningCosmWasmClient.connectWithSigner(
      `http://127.0.0.1:${context.park.ports.neutron.rpc}`,
      context.wallet,
      {
        gasPrice: GasPrice.fromString('0.025untrn'),
      },
    );
    context.neutronClient = new NeutronClient({
      apiURL: `http://127.0.0.1:${context.park.ports.neutron.rest}`,
      rpcURL: `127.0.0.1:${context.park.ports.neutron.rpc}`,
      prefix: 'neutron',
    });
  });

  afterAll(async () => {
    await context.park.stop();
  });

  it('upload contracts', async () => {
    const { client, account } = context;
    const res = await client.upload(
      account.address,
      Uint8Array.from(
        fs.readFileSync(
          join(
            __dirname,
            '../../../artifacts/maxbtc_neutron_coinfactory_wrapper.wasm',
          ),
        ),
      ),
      1.5,
    );
    expect(res.codeId).toBeGreaterThan(0);

    const instantiateRes = await CoinfactoryWrapper.instantiate(
      client,
      account.address,
      res.codeId,
      {
        owner: context.account.address,
        in_denom: DEPOSIT_DENOM,
        subdenom: 'subdenom',
        token_metadata: {
          exponent: 6,
          display: 'subdenom',
          name: 'subdenom',
          description: '',
          symbol: 'SUBDENOM',
          uri: '',
          uri_hash: '',
        },
      },
      'drop-staking-factory',
      'auto',
      [],
    );
    expect(instantiateRes.contractAddress).toHaveLength(66);
    context.coinfactoryWrapper = new CoinfactoryWrapper(
      client,
      instantiateRes.contractAddress,
    );
    context.coinfactoryDenom = `coinfactory.${instantiateRes.contractAddress}.subdenom`;
  });

  describe('wrap -> unwrap', () => {
    it('wrap', async () => {
      const { coinfactoryWrapper, account, client } = context;
      const balanceBefore = await client.getBalance(
        account.address,
        context.coinfactoryDenom,
      );
      await coinfactoryWrapper.wrap(account.address, 'auto', undefined, [
        {
          denom: DEPOSIT_DENOM,
          amount: '123',
        },
      ]);
      const balanceAfter = await client.getBalance(
        account.address,
        context.coinfactoryDenom,
      );
      expect(Number(balanceBefore.amount)).toBe(
        Number(balanceAfter.amount) - 123,
      );
    });

    it('[WrongDenom] wrap', async () => {
      const { coinfactoryWrapper, account, coinfactoryDenom } = context;
      const res = coinfactoryWrapper.wrap(account.address, 'auto', undefined, [
        {
          denom: coinfactoryDenom,
          amount: '123',
        },
      ]);
      await expect(res).rejects.toThrow(/Must send reserve token/);
    });
  });
});
