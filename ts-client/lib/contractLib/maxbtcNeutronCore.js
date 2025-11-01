"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
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
        return new Error("This client is not a SigningCosmWasmClient");
    }
    static async instantiate(client, sender, codeId, initMsg, label, fees, initCoins, admin) {
        const res = await client.instantiate(sender, codeId, initMsg, label, fees, {
            ...(initCoins && initCoins.length && { funds: initCoins }), ...(admin && { admin: admin }),
        });
        return res;
    }
    static async instantiate2(client, sender, codeId, salt, initMsg, label, fees, initCoins, admin) {
        const res = await client.instantiate2(sender, codeId, salt, initMsg, label, fees, {
            ...(initCoins && initCoins.length && { funds: initCoins }), ...(admin && { admin: admin }),
        });
        return res;
    }
    queryContractState = async () => {
        return this.client.queryContractSmart(this.contractAddress, { contract_state: {} });
    };
    queryActiveBatch = async () => {
        return this.client.queryContractSmart(this.contractAddress, { active_batch: {} });
    };
    queryWithdrawingBatch = async () => {
        return this.client.queryContractSmart(this.contractAddress, { withdrawing_batch: {} });
    };
    queryFinalizedBatches = async (args) => {
        return this.client.queryContractSmart(this.contractAddress, { finalized_batches: args });
    };
    queryConfig = async () => {
        return this.client.queryContractSmart(this.contractAddress, { config: {} });
    };
    queryExchangeRate = async () => {
        return this.client.queryContractSmart(this.contractAddress, { exchange_rate: {} });
    };
    queryDepositBalance = async () => {
        return this.client.queryContractSmart(this.contractAddress, { deposit_balance: {} });
    };
    querySimulateDeposit = async (args) => {
        return this.client.queryContractSmart(this.contractAddress, { simulate_deposit: args });
    };
    queryOwnership = async () => {
        return this.client.queryContractSmart(this.contractAddress, { ownership: {} });
    };
    tick = async (sender, fee, memo, funds) => {
        if (!isSigningCosmWasmClient(this.client)) {
            throw this.mustBeSigningClient();
        }
        return this.client.execute(sender, this.contractAddress, this.tickMsg(), fee || "auto", memo, funds);
    };
    tickMsg = () => { return { tick: {} }; };
    deposit = async (sender, args, fee, memo, funds) => {
        if (!isSigningCosmWasmClient(this.client)) {
            throw this.mustBeSigningClient();
        }
        return this.client.execute(sender, this.contractAddress, this.depositMsg(args), fee || "auto", memo, funds);
    };
    depositMsg = (args) => { return { deposit: args }; };
    withdraw = async (sender, fee, memo, funds) => {
        if (!isSigningCosmWasmClient(this.client)) {
            throw this.mustBeSigningClient();
        }
        return this.client.execute(sender, this.contractAddress, this.withdrawMsg(), fee || "auto", memo, funds);
    };
    withdrawMsg = () => { return { withdraw: {} }; };
    updateConfig = async (sender, args, fee, memo, funds) => {
        if (!isSigningCosmWasmClient(this.client)) {
            throw this.mustBeSigningClient();
        }
        return this.client.execute(sender, this.contractAddress, this.updateConfigMsg(args), fee || "auto", memo, funds);
    };
    updateConfigMsg = (args) => { return { update_config: args }; };
    mintFee = async (sender, args, fee, memo, funds) => {
        if (!isSigningCosmWasmClient(this.client)) {
            throw this.mustBeSigningClient();
        }
        return this.client.execute(sender, this.contractAddress, this.mintFeeMsg(args), fee || "auto", memo, funds);
    };
    mintFeeMsg = (args) => { return { mint_fee: args }; };
    updateOwnership = async (sender, args, fee, memo, funds) => {
        if (!isSigningCosmWasmClient(this.client)) {
            throw this.mustBeSigningClient();
        }
        return this.client.execute(sender, this.contractAddress, this.updateOwnershipMsg(args), fee || "auto", memo, funds);
    };
    updateOwnershipMsg = (args) => { return { update_ownership: args }; };
}
exports.Client = Client;
