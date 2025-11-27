'use strict';
Object.defineProperty(exports, '__esModule', { value: true });
exports.Client = void 0;
function isSigningCosmWasmClient(client) {
  return 'execute' in client;
}
class Client {
  client;
  contractAddress;
  constructor(client, contractAddress) {
    this.client = client;
    this.contractAddress = contractAddress;
  }
  mustBeSigningClient() {
    return new Error('This client is not a SigningCosmWasmClient');
  }
  static async instantiate(
    client,
    sender,
    codeId,
    initMsg,
    label,
    fees,
    initCoins,
    admin,
  ) {
    const res = await client.instantiate(sender, codeId, initMsg, label, fees, {
      ...(initCoins && initCoins.length && { funds: initCoins }),
      ...(admin && { admin: admin }),
    });
    return res;
  }
  static async instantiate2(
    client,
    sender,
    codeId,
    salt,
    initMsg,
    label,
    fees,
    initCoins,
    admin,
  ) {
    const res = await client.instantiate2(
      sender,
      codeId,
      salt,
      initMsg,
      label,
      fees,
      {
        ...(initCoins && initCoins.length && { funds: initCoins }),
        ...(admin && { admin: admin }),
      },
    );
    return res;
  }
  queryGetTwaer = async () => {
    return this.client.queryContractSmart(this.contractAddress, {
      get_twaer: {},
    });
  };
  queryExchangeRate = async () => {
    return this.client.queryContractSmart(this.contractAddress, {
      exchange_rate: {},
    });
  };
  queryOwnership = async () => {
    return this.client.queryContractSmart(this.contractAddress, {
      ownership: {},
    });
  };
  updateExchangeRate = async (sender, args, fee, memo, funds) => {
    if (!isSigningCosmWasmClient(this.client)) {
      throw this.mustBeSigningClient();
    }
    return this.client.execute(
      sender,
      this.contractAddress,
      this.updateExchangeRateMsg(args),
      fee || 'auto',
      memo,
      funds,
    );
  };
  updateExchangeRateMsg = (args) => {
    return { update_exchange_rate: args };
  };
  updateAum = async (sender, args, fee, memo, funds) => {
    if (!isSigningCosmWasmClient(this.client)) {
      throw this.mustBeSigningClient();
    }
    return this.client.execute(
      sender,
      this.contractAddress,
      this.updateAumMsg(args),
      fee || 'auto',
      memo,
      funds,
    );
  };
  updateAumMsg = (args) => {
    return { update_aum: args };
  };
  updateOwnership = async (sender, args, fee, memo, funds) => {
    if (!isSigningCosmWasmClient(this.client)) {
      throw this.mustBeSigningClient();
    }
    return this.client.execute(
      sender,
      this.contractAddress,
      this.updateOwnershipMsg(args),
      fee || 'auto',
      memo,
      funds,
    );
  };
  updateOwnershipMsg = (args) => {
    return { update_ownership: args };
  };
}
exports.Client = Client;
