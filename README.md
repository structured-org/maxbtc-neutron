# maxBTC Neutron Minting Contract

Welcome to the **maxbtc-neutron-minting** contract – the on-chain engine that mints, burns and settles **maxBTC**, a wrapped‐BTC token native to the [Neutron](https://neutron.org) ecosystem and secured by **CosmWasm** smart contracts.

> **TL;DR**
> *Users deposit BTC-backed collateral, receive freshly-minted `maxBTC`, and can later redeem it for native BTC through time-boxed withdrawal batches.
> Operators can flush deposits to external custody or manage a liquidation buffer, while the contract keeps a transparent, oracle-driven AUM ↔ maxBTC exchange-rate.*

---

## Table of Contents

1. [Motivation](#motivation)
2. [High-level Flow](#high-level-flow)
3. [Architecture](#architecture)
4. [Message Reference](#message-reference)

    * [Instantiate](#instantiate)
    * [Execute](#execute)
    * [Query](#query)
5. [Building & Testing](#building--testing)
6. [Running a Local Demo](#running-a-local-demo)
7. [Design Notes](#design-notes)
8. [Security & Audits](#security--audits)

---

## Motivation

Bridging BTC into the Cosmos is tricky: custodial bridges are opaque, while trust-minimised bridges remain nascent. **maxBTC** aims for a pragmatic middle-ground:

* **Full Reserve** –  every minted `maxBTC` is backed 1 : 1 by BTC in independent, auditable custody.
* **Batch Withdrawals** – users burn `maxBTC`, receive **redemption tokens**, and later claim native BTC once the batch is funded on the Bitcoin side.
* **Safety Valves** – a **liquidation buffer** ensures fast redemptions and guards against insolvency, while an emergency FSM can halt the protocol if invariants break.

The contract you see here coordinates these flows on Neutron.

---

## High-level Flow

1. **Deposit** – Users send the configured `deposit_denom` (e.g. `wBTC`) and instantly receive `maxBTC` minus a fee.
2. **FlushDeposits** – Anyone may flush accumulated deposits:
    * A share is routed to the **liquidation buffer** (`liquidation_buffer_share` of AUM).
    * The rest is IBC-transferred to external BTC custody via a dedicated “deposit pump” contract.
3. **Withdraw [DISABLED]** – Users burn `maxBTC` and receive *redemption tokens* tied to the current batch.
4. **ProcessActiveBatch [DISABLED]** – Once the batch’s active period expires, it transitions to **Withdrawing**.
5. **Claim [DISABLED]** – After the BTC lands in the custody address, users redeem their share by burning redemption tokens.

Throughout, the contract caches an **exchange-rate (ER)** snapshot to bridge multi-block operations safely.

---

## Architecture

### Exchange-Rate formula

```text
ER = (oracle_AUM + deposit_buffer + liquidation_buffer)
     ----------------------------------------------------
     (circulating_maxBTC + btc_requested – maxBTC_in_liquidation_buffer)
```

If the denominator is zero (bootstrap phase) the contract returns `1` to avoid division by zero.

### Finite-State Machine (FSM)

| State                       | Trigger In         | Trigger Out                 | Purpose                                     |
| ----------------------------| ------------------ |-----------------------------| ------------------------------------------- |
| **Idle**                    | –                  | Deposit / Withdraw / Flush  | Normal operation                            |
| **Flushing**                | FlushDeposits      | Money reached remote chain  | Ensures external custody received the funds |
| **Withdrawing [DISABLED]**  | ProcessActiveBatch | BTC collected (≈ requested) | Waits for BTC top-up before claims          |

---

## Message Reference

### Execute

| Variant                 | Who can call | Description                                           |
| ----------------------- | ------------ | ----------------------------------------------------- |
| `UpdateConfig`          | **Owner**    | Fine-grained config updates                           |
| `Deposit { recipient }` | Anyone       | Deposit `deposit_denom` → receive `maxBTC`            |
| `FlushDeposits {}`      | Anyone       | Flush buffer; manages liquidation buffer & IBC pump   |
| `Withdraw {}`           | Anyone       | Burn `maxBTC` → mint redemption tokens                |
| `ProcessActiveBatch {}` | Anyone       | After `batch_active_duration`, start withdrawal phase |
| `Claim { recipient }`   | Anyone       | Claim BTC proportional to burned redemption tokens    |

### Query

| Query                         | Returns                 |
| ----------------------------- | ----------------------- |
| `Config {}`                   | `ConfigResponse`        |
| `ActiveBatch {}`              | `Option<BatchResponse>` |
| `WithdrawingBatch {}`         | `Option<BatchResponse>` |
| `FinalizedBatch { batch_id }` | `Option<BatchResponse>` |

---

## Design Notes

* **Tokenfactory Integration** – The contract creates its own `factory/<addr>/maxbtc` denom on instantiation and later mints/burns via native `MsgMint` & `MsgBurn`.
* **Safety Checks** –

    * Deposit cap (`deposits_cap`) prevents runaway growth.
    * Optional allow-list (`deposits_allowlist`) for regulated environments.
    * Every multi-step flow caches the ER + AUM snapshot and refuses further state-mutations if the cache expires (→ “emergency mode”).
* **Gas Efficiency** – Core bookkeeping (AUM, ER, batch maths) is done in-contract; cross-chain actions are batched into a single `flush` to reduce IBC overhead.

---

## Security & Audits

This repository has **not yet** undergone a formal security audit.
✔ Internal invariants are unit-tested and fuzz-tested.
⚠ **Main-net usage is discouraged until a full audit is complete.**

Audit status and reports will be tracked in the [Security](./SECURITY.md) section.

---

## Contributing

Bug reports, feature ideas and PRs are welcome!
Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for coding standards and the DCO sign-off procedure.
