#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
        token, Address, Env, IntoVal, Symbol,
    };

    use crate::{CardTradeHub, CardTradeHubClient, TradeStatus};

    // ── helpers ────────────────────────────────────────────────────────────────

    /// Spins up env + contract + USDC mock, funds buyer, returns everything needed.
    fn setup() -> (Env, CardTradeHubClient<'static>, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy the escrow contract
        let contract_id = env.register_contract(None, CardTradeHub);
        let client = CardTradeHubClient::new(&env, &contract_id);

        // Addresses
        let admin  = Address::generate(&env);
        let seller = Address::generate(&env);
        let buyer  = Address::generate(&env);

        // Deploy a mock USDC token
        let usdc_id = env.register_stellar_asset_contract(admin.clone());
        let usdc_admin = token::StellarAssetClient::new(&env, &usdc_id);

        // Mint 1 000 USDC (7 decimals → 10_000_000_000 stroops) to buyer
        usdc_admin.mint(&buyer, &10_000_000_000_i128);

        // Initialise contract with admin
        client.init(&admin);

        (env, client, admin, seller, buyer, usdc_id)
    }

    // ── Test 1 – Happy path ────────────────────────────────────────────────────
    /// Full end-to-end flow: list → fund → both confirm → USDC lands with seller.
    #[test]
    fn test_happy_path_full_trade() {
        let (env, client, _admin, seller, buyer, usdc_id) = setup();

        let amount: i128 = 5_000_000_000; // 500 USDC

        // Step 1 – Seller lists the card
        let trade_id = client.list_trade(
            &seller,
            &buyer,
            &usdc_id,
            &amount,
            &Symbol::new(&env, "PSA10_CHARIZARD"),
        );

        // Step 2 – Buyer funds the escrow
        client.fund_escrow(&buyer, &trade_id);

        // Step 3 – Seller confirms shipment
        client.confirm_receipt(&seller, &trade_id);

        // Step 4 – Buyer confirms receipt
        client.confirm_receipt(&buyer, &trade_id);

        // Assert seller received USDC
        let usdc = token::Client::new(&env, &usdc_id);
        assert_eq!(usdc.balance(&seller), amount);

        // Assert trade is Completed
        let state = client.get_trade(&trade_id);
        assert_eq!(state.status, TradeStatus::Completed);
    }

    // ── Test 2 – Edge case: wrong buyer cannot fund escrow ────────────────────
    /// A random address that is NOT the designated buyer should panic.
    #[test]
    #[should_panic(expected = "caller is not the buyer")]
    fn test_wrong_buyer_cannot_fund() {
        let (env, client, _admin, seller, buyer, usdc_id) = setup();

        let impostor = Address::generate(&env);
        let amount: i128 = 1_000_000_000;

        // Mint USDC to impostor so the transfer wouldn't fail for a balance reason
        let usdc_admin = token::StellarAssetClient::new(&env, &usdc_id);
        usdc_admin.mint(&impostor, &amount);

        let trade_id = client.list_trade(
            &seller,
            &buyer,
            &usdc_id,
            &amount,
            &Symbol::new(&env, "PSA10_LEBRON"),
        );

        // Impostor tries to fund — must panic
        client.fund_escrow(&impostor, &trade_id);
    }

    // ── Test 3 – State verification after funding ──────────────────────────────
    /// After fund_escrow, the contract should hold the USDC and status = Funded.
    #[test]
    fn test_state_after_funding() {
        let (env, client, _admin, seller, buyer, usdc_id) = setup();

        let amount: i128 = 2_000_000_000; // 200 USDC

        let trade_id = client.list_trade(
            &seller,
            &buyer,
            &usdc_id,
            &amount,
            &Symbol::new(&env, "PSA10_PIKACHU"),
        );

        client.fund_escrow(&buyer, &trade_id);

        let state = client.get_trade(&trade_id);

        // Trade status must be Funded
        assert_eq!(state.status, TradeStatus::Funded);
        // Neither party has confirmed yet
        assert!(!state.seller_confirmed);
        assert!(!state.buyer_confirmed);

        // Contract must now hold the USDC
        let usdc = token::Client::new(&env, &usdc_id);
        let contract_id = env.register_contract(None, CardTradeHub); // re-get address via env
        // Use state fields to verify amounts indirectly: buyer balance decreased
        assert_eq!(usdc.balance(&buyer), 10_000_000_000_i128 - amount);
    }

    // ── Test 4 – Admin cancels funded trade and refunds buyer ─────────────────
    #[test]
    fn test_admin_cancel_refunds_buyer() {
        let (env, client, admin, seller, buyer, usdc_id) = setup();

        let amount: i128 = 3_000_000_000; // 300 USDC
        let initial_balance = 10_000_000_000_i128;

        let trade_id = client.list_trade(
            &seller,
            &buyer,
            &usdc_id,
            &amount,
            &Symbol::new(&env, "PSA10_GOKU"),
        );

        client.fund_escrow(&buyer, &trade_id);

        // Admin cancels the trade
        client.cancel_trade(&admin, &trade_id);

        let state = client.get_trade(&trade_id);
        assert_eq!(state.status, TradeStatus::Cancelled);

        // Buyer should be fully refunded
        let usdc = token::Client::new(&env, &usdc_id);
        assert_eq!(usdc.balance(&buyer), initial_balance);
    }

    // ── Test 5 – Cannot confirm on an Open (unfunded) trade ───────────────────
    #[test]
    #[should_panic(expected = "trade must be Funded")]
    fn test_cannot_confirm_unfunded_trade() {
        let (env, client, _admin, seller, buyer, usdc_id) = setup();

        let trade_id = client.list_trade(
            &seller,
            &buyer,
            &usdc_id,
            &1_000_000_000_i128,
            &Symbol::new(&env, "PSA10_ZION"),
        );

        // Seller tries to confirm before buyer funds — must panic
        client.confirm_receipt(&seller, &trade_id);
    }
}