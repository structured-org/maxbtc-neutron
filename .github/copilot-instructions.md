## Project Overview
This project contains MaxBTC contracts for Neutron blockchain.

## Rules
Every response must follow these rules:
1. Each response must start with: "OK:"

## Structure
`contracts/maxbtc-neutorn-core` - Token core contract
`contracts/maxbtc-neutorn-factory` - Factory contract
`contracts/maxbtc-neutron-allow-list` - Allow List contract mock
`contracts/maxbtc-neutron-exchange-rate-provider` - Exchange Rate Provider mock
`contracts/maxbtc-neutron-fee-collector` - Fee Collector
`contracts/maxbtc-neutron-withdrawal-notifier` - Withdrawal Notifier

## Contract structure
Each contract has the following structure:
- `src/` - Source code of the contract
- `src/msg.rs` - All messages (InstantiateMsg, ExecuteMsg, QueryMsg, etc.)
- `src/state.rs` - State definitions
- `src/testing` - Unit tests
or
- `src/tests.rs` - Unit tests