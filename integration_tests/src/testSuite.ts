import cosmopark, { CosmoparkConfig } from '@neutron-org/cosmopark';
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing';
import {
  StargateClient,
} from '@cosmjs/stargate';
import { waitFor } from './helpers/waitFor';
import fs from 'fs';
import {
  CosmoparkNetworkConfig,
} from '@neutron-org/cosmopark/lib/types';
import { Suite } from 'vitest';
const packageJSON = require(`${__dirname}/../package.json`);
const VERSION = (process.env.CI ? '_' : ':') + packageJSON.version;
const ORG = process.env.CI ? 'neutronorg/lionco-contracts:' : '';

const keys = [
  'master',
  'demowallet1',
  'demowallet2',
  'demo1',
  'demo2',
  'demo3',
] as const;

const redefinedParams =
    process.env.REMOTE_CHAIN_OPTS && fs.existsSync(process.env.REMOTE_CHAIN_OPTS)
        ? JSON.parse(fs.readFileSync(process.env.REMOTE_CHAIN_OPTS).toString())
        : {
          commands: {
            addGenesisAccount: 'genesis add-genesis-account',
            gentx: 'genesis gentx',
            collectGenTx: 'genesis collect-gentxs',
          },
        };

const networkConfigs = {
  neutron: {
    binary: 'neutrond',
    chain_id: 'ntrntest',
    denom: 'untrn',
    image: `${ORG}neutron-test${VERSION}`,
    prefix: 'neutron',
    loglevel: 'debug',
    trace: true,
    public: true,
    type: 'ics',
    upload: [
      './artifacts/contracts',
      './artifacts/contracts_thirdparty',
      './artifacts/scripts/init-neutrond.sh',
    ],
    post_init: ['CHAINID=ntrntest CHAIN_DIR=/opt /opt/init-neutrond.sh'],
    genesis_opts: {
      'app_state.crisis.constant_fee.denom': 'untrn',
    },
    config_opts: {
      'consensus.timeout_commit': '500ms',
      'consensus.timeout_propose': '500ms',
    },
    app_opts: {
      'api.enable': 'true',
      'api.address': 'tcp://0.0.0.0:1317',
      'api.swagger': 'true',
      'grpc.enable': 'true',
      'grpc.address': '0.0.0.0:9090',
      'minimum-gas-prices': '0.0025untrn',
      'rosetta.enable': 'true',
      'telemetry.prometheus-retention-time': 1000,
    },
  },
};

type Keys = (typeof keys)[number];

const awaitFirstBlock = (rpc: string): Promise<void> =>
    waitFor(async () => {
      try {
        const controller = new AbortController();
        setTimeout(() => controller.abort(), 1000);
        await fetch(rpc, {
          method: 'GET',
          signal: controller.signal,
        });
        const client = await StargateClient.connect(rpc);
        const block = await client.getBlock();
        if (block.header.height > 1) {
          return true;
        }
      } catch (e) {
        return false;
      }
    }, 20_000);

export const generateWallets = (): Promise<Record<Keys, string>> =>
    keys.reduce(
        async (acc, key) => {
          const accObj = await acc;
          const wallet = await DirectSecp256k1HdWallet.generate(12, {
            prefix: 'neutron',
          });
          accObj[key] = wallet.mnemonic;
          return accObj;
        },
        Promise.resolve({} as Record<Keys, string>),
    );

type NetworkOptsType = Partial<Record<keyof typeof networkConfigs | '*', any>>;
const getNetworkConfig = (
    network: string,
    opts: NetworkOptsType = {},
): CosmoparkNetworkConfig => {
  let config = networkConfigs[network];
  const extOpts = opts['*'] || opts[network] || {};
  for (const [key, value] of Object.entries(extOpts)) {
    if (typeof value === 'object') {
      config = { ...config, [key]: { ...config[key], ...value } };
    } else {
      config = { ...config, [key]: value };
    }
  }
  return config;
};

function isSuite(t: any): t is Suite {
  return t && t.type === 'suite' && t.suite;
}

export const setupPark = async (
    t: Readonly<Suite | File>,
    networks: string[] = [],
    opts?: NetworkOptsType, // Key is path to the param, value is Record of network name and value
): Promise<cosmopark> => {
  const context = ((t: Readonly<Suite | File>) => {
    if (isSuite(t)) {
      return t.suite.file.filepath
          ?.split('/')
          .pop()!
          .split('.')[0]
          .replace(/[-_]/g, '');
    } else {
      throw new Error('Invalid context');
    }
  })(t);
  const wallets = await generateWallets();
  const config: CosmoparkConfig = {
    context,
    networks: {},
    master_mnemonic: wallets.master,
    loglevel: 'info',
    wallets: {
      demowallet1: {
        mnemonic: wallets.demowallet1,
        balance: '1000000000',
      },
      demowallet2: {
        mnemonic: wallets.demowallet2,
        balance: '1000000000',
      },
      demo1: { mnemonic: wallets.demo1, balance: '1000000000' },
      demo2: { mnemonic: wallets.demo2, balance: '1000000000' },
      demo3: { mnemonic: wallets.demo3, balance: '1000000000' },
    },
  };
  for (const network of networks) {
    config.networks[network] = getNetworkConfig(network, opts);
  }
  config.relayers = [];
  const instance = await cosmopark.create(config);
  await Promise.all(
      Object.entries(instance.ports).map(([network, ports]) =>
          awaitFirstBlock(`http://127.0.0.1:${ports.rpc}`).catch((e) => {
            console.log(`Failed to await first block for ${network}: ${e}`);
            throw e;
          }),
      ),
  );
  return instance;
};
