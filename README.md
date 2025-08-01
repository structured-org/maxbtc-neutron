# Core contract

## Core Functionality: Depositing and Minting

The main purpose of the contract is to allow users to deposit a specific token (assuming `wBTC`) and mint `maxBTC`.

Here’s how the deposit process works from a user's perspective:

1. **Initiate Deposit:** A user sends a transaction to the contract's `Deposit` function, including the funds they wish to deposit. The recipient of the new `maxBTC` can be the sender or another specified address.
2. **Perform Checks:** Before proceeding, the contract runs several critical safety checks:
    - **Is the contract paused?** The owner can temporarily halt all deposit activity.
    - **Is the deposit cap reached?** The contract tracks the total value of assets deposited. If a new deposit would exceed a predefined `deposits_cap`, it's rejected to manage risk.
    - **Is the recipient allowlisted?** The contract checks with a separate `allowlist_contract` to ensure the address designated *to receive* the `maxBTC` is authorized for deposits.
3. **Calculate Mint Amount:** If the checks pass, the contract calculates how much `maxBTC` to mint. This isn't a 1-to-1 conversion. The calculation is:
    - It queries a separate, trusted `exchange_rate_provider_contract` to get the latest price of the deposited asset relative to `maxBTC`.
    - It subtracts a small, fixed percentage fee (`deposit_cost`) from the user's deposit.
    - The final amount of `maxBTC` to be minted is essentially `(Deposited Amount - Fee) / Exchange Rate`.
4. **Mint and Deliver `maxBTC`:** the calculated amount of `maxBTC` is minted and delivered directly to the recipient's wallet. The contract then updates its internal **counter** for the total amount deposited.

## Flushing Deposits

The assets that users deposit (e.g., `wBTC`) accumulate in the contract's balance. The `FlushDeposits` function exists to move these funds to their next destination.

- **Who can trigger it?** Anyone can call this function.
- **How it works:** It's rate-limited by a `deposit_flush_period` (e.g., every 24 hours). When triggered after the cooldown has passed, the contract takes its entire balance of the deposit asset and sends it to a `deposit_forwarder_contract`. This "`forwarder`" contract is responsible for the next step, such as bridging the assets to Ethereum via IBC Eureka.
- **Purpose:** This mechanism ensures the underlying assets that back `maxBTC` are regularly moved to where they can be put to work.

## Fee Collection

There are two layers to fees. The first is the simple `deposit_cost` taken from each deposit. The second involves a companion contract.

The main contract has a dedicated `fee_collector_contract` that it works with. This fee collector can request a special minting operation via the `MintFee` function. Only the address of the fee collector is authorized to call this function. When called, the main contract will mint a specified amount of `maxBTC` and send it to the fee collector. This allows the protocol to create `maxBTC` as a form of revenue or reward for the fee collector, which can then manage or distribute those funds according to its own logic. See the dedicated section below.

## Contract Setup and Administration

### Initial Setup (Instantiation)

When the contract is first deployed, it performs a one-time setup sequence:

1. **Create the `maxBTC` Denom:** Its very first action is to use the Token Factory to officially create the `maxBTC` token on the Neutron blockchain. From that point on, this contract becomes the exclusive minter for `maxBTC`.
2. **Deploy the Fee Collector:** The contract uses a feature called `Instantiate2` to **predict the blockchain address** of its companion `fee_collector_contract` *before* it exists.
3. **Launch the Fee Collector:** With the address predicted, it immediately sends a message to create the `fee_collector_contract` at that known address, providing it with the necessary information to link the two contracts together from birth.

### Owner Controls

The contract owner has significant control over its parameters through the `UpdateConfig` function. This is a permissioned function that only the owner can call. The owner can:

- Pause or unpause the contract.
- Change the owner to a new address.
- Update the addresses of its connected contracts (the allowlist, exchange rate provider, deposit forwarder, and fee collector).
- Adjust the deposit cap and the flush period.

## Information Queries (Read-Only)

Users can query the contract to get information without needing to send a transaction. The most important queries are:

- **`Config`:** Displays the current configuration, such as the owner and key contract addresses.
- **`ExchangeRate`:** Fetches and returns the current exchange rate from the provider contract.
- **`SimulateDeposit`:** Allows a user to input a potential deposit amount and see exactly how much `maxBTC` they would receive in return, after fees and the exchange rate are applied. This is a useful tool for users to check the outcome before committing funds.

# Fee Collector Contract

This contract is a specialized companion to the main `maxBTC` minting contract. Its single, crucial job is to calculate and collect a performance-based fee by minting new `maxBTC`. It effectively captures a percentage of the yield that `maxBTC` generates over time.

## Core Mission: Collecting Fees on Yield

The central function of the contract is `CollectFee`. While anyone can trigger this function, it will only execute under specific conditions.

1. **Cooldown Period:** The contract will only run if a set amount of time (the `collection_period_seconds`) has passed since the last time a fee was collected. This prevents it from being triggered too frequently.
2. **Positive Yield Check:** The contract compares the **current** exchange rate of `maxBTC` with the rate it recorded during the **last** collection. **If the rate has not increased, it means `maxBTC` has not generated any yield, and the contract will do nothing.** No profit means no fee is taken.

If both conditions are met, the contract proceeds to calculate the fee and instructs the main `maxBTC` minting contract to create new tokens for it.

## How the Fee is Calculated

The fee calculation is sophisticated. Instead of taking a simple cut of transactions, it captures a portion of the overall system's growth (its APY).

Here’s the concept:

- The contract determines how much the `maxBTC` exchange rate has appreciated since the last collection. This appreciation is the "yield."
- Based on a configured `fee_apy_reduction_percentage`, it calculates how many new `maxBTC` tokens it needs to mint to effectively "skim off" a percentage of that yield.

This newly minted `maxBTC` is sent to and held by this fee collector contract.

## The Two-Step Update: A Failsafe Mechanism

The process of collecting the fee is a clever two-step dance to ensure it never fails halfway.

1. **Step 1 (Request):** The fee collector sends a request to the main contract, asking it to mint the calculated fee amount.
2. **Step 2 (Reply):** The fee collector then waits for a confirmation, or `reply`, from the main contract. **Only after it receives confirmation that the minting was successful does it update its own internal records.** It saves the new, slightly diluted exchange rate and resets the timestamp for the next collection period.

This two-step process is a critical failsafe. It guarantees that the fee collector only updates its state if the fee was actually collected, preventing it from getting out of sync with the main system.

## Payouts and Administration

Once the fees are collected, they need to be managed.

- **Claiming Fees:** The contract owner can call the `Claim` function. This is a permissioned, owner-only action that sends the `maxBTC` tokens accumulated in the fee collector's balance to any specified recipient address. This is how the protocol's revenue is ultimately paid out.
- **Configuration:** The owner can also update the contract's parameters, such as changing the fee percentage, the length of the collection period, or the address of the main `core_contract` it communicates with.

# Allow List Contract

This smart contract is a streamlined access control utility for the `maxBTC` protocol. Its sole function is to maintain and enforce a definitive list of authorized addresses, acting as a simple and secure gatekeeper for key system actions.

### Core Functionality and Verification

The contract's mechanism is based on a single, centrally managed allow-list. This list contains all blockchain addresses that are pre-approved for participation.

The primary purpose of this contract is to answer one question for other contracts in the ecosystem: "Is a given address allowed?" This is handled by the `IsAddressAllowed` query. If a match is found, the address is considered authorized. If not, it is unauthorized.

### Secure Administration and Management

The integrity of the allow-list is protected by a strict ownership model. The contract is controlled by a single owner, and critically, **only the owner** has the permission to modify the list.

This administrative control is exercised through the `UpdateAllowList` function, which allows the owner to replace the entire existing list with a new one. This ensures that the list cannot be tampered with by unauthorized parties. The contract owner can also securely transfer ownership to a new address.