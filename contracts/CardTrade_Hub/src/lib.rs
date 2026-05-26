#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Symbol,
};

// ── Storage Keys ─────────────────────────────────────────────────────────────

/// All persistent state keys used by the escrow contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Trade(u64),      // TradeState keyed by trade_id
    NextTradeId,     // Auto-increment counter for trade IDs
}

// ── Trade State ───────────────────────────────────────────────────────────────

/// Every possible state a trade can be in.
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum TradeStatus {
    /// Seller listed card; waiting for buyer to fund escrow.
    Open,
    /// Buyer deposited USDC; waiting for both parties to confirm receipt.
    Funded,
    /// Both parties confirmed; USDC released to seller.
    Completed,
    /// Admin cancelled the trade and refunded the buyer (clawback path).
    Cancelled,
}

/// Full on-chain state for a single trade.
#[contracttype]
#[derive(Clone)]
pub struct TradeState {
    pub trade_id:          u64,
    pub seller:            Address,
    pub buyer:             Address,
    pub usdc_token:        Address,   // USDC token contract address
    pub amount:            i128,      // Amount in USDC stroops (7 decimals)
    pub status:            TradeStatus,
    pub seller_confirmed:  bool,      // Seller ticked "card shipped / delivered"
    pub buyer_confirmed:   bool,      // Buyer ticked "card received & verified"
    pub card_description:  Symbol,    // e.g. "PSA10_CHARIZARD"
}

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct CardTradeHub;

#[contractimpl]
impl CardTradeHub {

    // ── 1. list_trade ─────────────────────────────────────────────────────────
    /// Seller calls this to list a card for trade.
    /// Creates a new TradeState in Open status and returns the trade_id.
    pub fn list_trade(
        env:              Env,
        seller:           Address,
        buyer:            Address,
        usdc_token:       Address,
        amount:           i128,
        card_description: Symbol,
    ) -> u64 {
        // Seller must authorise this call (prevents spoofing)
        seller.require_auth();

        // amount must be positive
        assert!(amount > 0, "amount must be positive");

        // Assign a unique trade ID
        let trade_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextTradeId)
            .unwrap_or(0u64);

        let state = TradeState {
            trade_id,
            seller:           seller.clone(),
            buyer:            buyer.clone(),
            usdc_token,
            amount,
            status:           TradeStatus::Open,
            seller_confirmed: false,
            buyer_confirmed:  false,
            card_description,
        };

        // Persist state and bump the counter
        env.storage().instance().set(&DataKey::Trade(trade_id), &state);
        env.storage().instance().set(&DataKey::NextTradeId, &(trade_id + 1));

        env.events().publish(
            (Symbol::new(&env, "trade_listed"), trade_id),
            seller,
        );

        trade_id
    }

    // ── 2. fund_escrow ────────────────────────────────────────────────────────
    /// Buyer calls this to deposit USDC into the contract (escrow lock).
    /// Trade must be in Open status.
    /// After this call the trade moves to Funded.
    pub fn fund_escrow(env: Env, buyer: Address, trade_id: u64) {
        buyer.require_auth();

        let mut state: TradeState = env
            .storage()
            .instance()
            .get(&DataKey::Trade(trade_id))
            .expect("trade not found");

        assert!(state.status == TradeStatus::Open,  "trade must be Open");
        assert!(state.buyer == buyer,               "caller is not the buyer");

        // Transfer USDC from buyer → this contract (escrow lock)
        let token_client = token::Client::new(&env, &state.usdc_token);
        token_client.transfer(
            &buyer,
            &env.current_contract_address(),
            &state.amount,
        );

        state.status = TradeStatus::Funded;
        env.storage().instance().set(&DataKey::Trade(trade_id), &state);

        env.events().publish(
            (Symbol::new(&env, "escrow_funded"), trade_id),
            buyer,
        );
    }

    // ── 3. confirm_receipt ────────────────────────────────────────────────────
    /// Either party calls this to confirm their side of the trade.
    /// - Seller confirms when card has been shipped / handed over.
    /// - Buyer confirms when card is physically received and verified.
    /// When BOTH have confirmed, USDC is released to the seller automatically.
    pub fn confirm_receipt(env: Env, caller: Address, trade_id: u64) {
        caller.require_auth();

        let mut state: TradeState = env
            .storage()
            .instance()
            .get(&DataKey::Trade(trade_id))
            .expect("trade not found");

        assert!(state.status == TradeStatus::Funded, "trade must be Funded");

        // Set the correct confirmation flag
        if caller == state.seller {
            state.seller_confirmed = true;
        } else if caller == state.buyer {
            state.buyer_confirmed = true;
        } else {
            panic!("caller is not a party to this trade");
        }

        // If both parties confirmed → release funds to seller
        if state.seller_confirmed && state.buyer_confirmed {
            let token_client = token::Client::new(&env, &state.usdc_token);
            token_client.transfer(
                &env.current_contract_address(),
                &state.seller,
                &state.amount,
            );
            state.status = TradeStatus::Completed;

            env.events().publish(
                (Symbol::new(&env, "trade_completed"), trade_id),
                state.seller.clone(),
            );
        }

        env.storage().instance().set(&DataKey::Trade(trade_id), &state);
    }

    // ── 4. cancel_trade (admin clawback) ─────────────────────────────────────
    /// Admin-only: cancel a Funded trade and refund USDC to the buyer.
    /// This is the Stellar Clawback safety valve for dispute resolution.
    pub fn cancel_trade(env: Env, admin: Address, trade_id: u64) {
        admin.require_auth();

        // Only the contract deployer (stored as "admin") can call this
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "admin"))
            .expect("admin not initialised — call init first");

        assert!(admin == stored_admin, "caller is not admin");

        let mut state: TradeState = env
            .storage()
            .instance()
            .get(&DataKey::Trade(trade_id))
            .expect("trade not found");

        assert!(state.status == TradeStatus::Funded, "can only cancel a Funded trade");

        // Refund USDC from escrow back to buyer
        let token_client = token::Client::new(&env, &state.usdc_token);
        token_client.transfer(
            &env.current_contract_address(),
            &state.buyer,
            &state.amount,
        );

        state.status = TradeStatus::Cancelled;
        env.storage().instance().set(&DataKey::Trade(trade_id), &state);

        env.events().publish(
            (Symbol::new(&env, "trade_cancelled"), trade_id),
            state.buyer,
        );
    }

    // ── 5. init ───────────────────────────────────────────────────────────────
    /// One-time initialisation: sets the admin address.
    /// Must be called immediately after deployment.
    pub fn init(env: Env, admin: Address) {
        // Prevent re-initialisation
        assert!(
            !env.storage().instance().has(&Symbol::new(&env, "admin")),
            "already initialised"
        );
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "admin"), &admin);
    }

    // ── 6. get_trade ─────────────────────────────────────────────────────────
    /// Read-only: returns the full TradeState for a given trade_id.
    pub fn get_trade(env: Env, trade_id: u64) -> TradeState {
        env.storage()
            .instance()
            .get(&DataKey::Trade(trade_id))
            .expect("trade not found")
    }
}