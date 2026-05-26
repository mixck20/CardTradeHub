<img width="1920" height="911" alt="image" src="https://github.com/user-attachments/assets/41ad9afe-b957-4a3b-b3a8-6e514000c083" /># CardTrade Hub

> Trustless USDC escrow for graded collectible card trading on Stellar

---

## Problem

College students and young collectors in the Philippines trade expensive graded cards (PSA 10 Pokémon, NBA, One Piece) through Facebook groups with **zero safety net**. Scammers disappear after payment, or ship fake slabs. A single bad deal can cost ₱2,000–₱50,000+.

## Solution

CardTrade Hub locks USDC in a **Soroban smart-contract escrow**. Funds only move when **both** the seller and the buyer confirm the physical card changed hands — making front-running and ghost-shipping scams impossible.

---

## Core Trade Flow (MVP — 2 minutes end-to-end)

```
Seller  ──list_trade──▶  Contract (Open)
Buyer   ──fund_escrow──▶ Contract (Funded)  ← USDC locked here
Seller  ──confirm_receipt──▶ Contract
Buyer   ──confirm_receipt──▶ Contract (Completed) ── USDC ──▶ Seller
```

---

## Stellar Features Used

| Feature | How CardTrade Hub uses it |
|---|---|
| **USDC transfers** | Primary settlement currency for all escrow deposits and releases |
| **Soroban smart contracts** | Escrow logic, mutual-confirmation gate, state machine |
| **Trustlines** | Each user wallet must have a USDC trustline before funding |
| **Clawback** | Admin `cancel_trade` returns funds to buyer in dispute scenarios |

---

## Vision & Purpose

CardTrade Hub turns every Facebook card deal into a trustless, on-chain transaction — giving Filipino collectors the same safety that credit-card chargebacks give mainstream e-commerce buyers, without needing a bank.

---

## Timeline

| Phase | Milestone |
|---|---|
| Week 1 | Smart contract + 5-test suite complete |
| Week 2 | Testnet deployment + CLI demo |
| Week 3 | React frontend (wallet connect, list/fund/confirm UI) |
| Week 4 | AI card-slab verification bonus feature |

---

## Prerequisites

- **Rust** ≥ 1.74 with `wasm32-unknown-unknown` target
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- **Stellar CLI** ≥ 0.9.4
  ```bash
  cargo install --locked stellar-cli --features opt
  ```
- A **Stellar testnet** account funded via Friendbot

---

## Build

```bash
# Compile to optimised Wasm
stellar contract build
# Output: target/wasm32-unknown-unknown/release/cardtrade_hub.wasm
```

---

## Test

```bash
cargo test
```

Expected output:
```
running 5 tests
test tests::test_happy_path_full_trade      ... ok
test tests::test_wrong_buyer_cannot_fund    ... ok
test tests::test_state_after_funding        ... ok
test tests::test_admin_cancel_refunds_buyer ... ok
test tests::test_cannot_confirm_unfunded_trade ... ok

test result: ok. 5 passed; 0 failed
```

---

## Deploy to Testnet

```bash
# 1. Generate / fund a deployer keypair
stellar keys generate deployer --network testnet
stellar keys fund deployer --network testnet

# 2. Deploy the Wasm
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/cardtrade_hub.wasm \
  --source deployer \
  --network testnet

# Save the returned CONTRACT_ID

# 3. Initialise the admin
stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- init \
  --admin $(stellar keys address deployer)
```

---

## Sample CLI Invocations

### List a trade (seller)
```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source seller_key \
  --network testnet \
  -- list_trade \
  --seller  GSELLER... \
  --buyer   GBUYER... \
  --usdc_token GUSDC_CONTRACT... \
  --amount  5000000000 \
  --card_description PSA10_CHARIZARD
```

### Fund escrow (buyer)
```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source buyer_key \
  --network testnet \
  -- fund_escrow \
  --buyer    GBUYER... \
  --trade_id 0
```

### Confirm receipt (each party)
```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source seller_key \
  --network testnet \
  -- confirm_receipt \
  --caller   GSELLER... \
  --trade_id 0

stellar contract invoke \
  --id $CONTRACT_ID \
  --source buyer_key \
  --network testnet \
  -- confirm_receipt \
  --caller   GBUYER... \
  --trade_id 0
# ✅ USDC released to seller automatically after both confirm
```

### Cancel trade — admin only (dispute / clawback)
```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- cancel_trade \
  --admin    GADMIN... \
  --trade_id 0
```

### Read trade state
```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --network testnet \
  -- get_trade \
  --trade_id 0
```

---

## Reference Repos

- Deployment guide: https://github.com/armlynobinguar/Stellar-Bootcamp-2026
- Full-stack example: https://github.com/armlynobinguar/community-treasury

---

## Contract ID
CA2RSKKIXJY4GLWRSMRQAFATWPXML2NJGEPOFHZVLLKHGU3BLVC5I7M3

## License

MIT © 2026 CardTrade Hub
