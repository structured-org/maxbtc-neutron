#[cfg(test)]
mod helpers_test {
    use std::str::FromStr;

    use crate::helpers::*;
    use cosmwasm_std::{testing::mock_env, Addr, BankMsg, CosmosMsg, Decimal, Uint128};

    fn create_mock_env() -> cosmwasm_std::Env {
        let mut env = mock_env();
        env.contract.address = Addr::unchecked("contract");
        env
    }

    // Helper function to extract send amount from bank message
    fn extract_send_amount_and_recipient(msg: &CosmosMsg) -> Option<(Uint128, String)> {
        if let CosmosMsg::Bank(BankMsg::Send { to_address, amount }) = msg {
            if !amount.is_empty() {
                return Some((amount[0].amount, to_address.clone()));
            }
        }
        None
    }

    // Helper function to verify message types and extract amounts
    fn verify_burn_and_send_messages(
        messages: &[CosmosMsg],
        expected_send_amount: Uint128,
        expected_recipient: &str,
        expected_denom: &str,
    ) -> (bool, bool) {
        if messages.len() != 2 {
            return (false, false);
        }

        // First message should be burn (any non-Bank message)
        let burn_msg_valid = !matches!(messages[0], CosmosMsg::Bank(_));

        // Second message should be Bank::Send with correct amount
        let send_msg_valid =
            if let Some((amount, recipient)) = extract_send_amount_and_recipient(&messages[1]) {
                amount == expected_send_amount && recipient == expected_recipient
            } else {
                false
            };

        (burn_msg_valid, send_msg_valid)
    }

    #[test]
    fn test_get_deposit_price_success() {
        let exchange_rate = Decimal::from_atomics(100u128, 0).unwrap(); // 100
        let price_multiplier_basis_points = 500u128; // 5%

        let result = get_deposit_price(exchange_rate, price_multiplier_basis_points);

        assert!(result.is_ok());
        let deposit_price = result.unwrap();
        // Expected: 100 * (1 + 0.05) = 105
        let expected = Decimal::from_atomics(105u128, 0).unwrap();
        assert_eq!(deposit_price, expected);
    }

    #[test]
    fn test_get_deposit_price_zero_multiplier() {
        let exchange_rate = Decimal::from_atomics(100u128, 0).unwrap();
        let price_multiplier_basis_points = 0u128; // 0%

        let result = get_deposit_price(exchange_rate, price_multiplier_basis_points);

        assert!(result.is_ok());
        let deposit_price = result.unwrap();
        // Expected: 100 * (1 + 0) = 100
        assert_eq!(deposit_price, exchange_rate);
    }

    #[test]
    fn test_get_deposit_price_high_multiplier() {
        let exchange_rate = Decimal::from_atomics(100u128, 0).unwrap();
        let price_multiplier_basis_points = 10000u128; // 100%

        let result = get_deposit_price(exchange_rate, price_multiplier_basis_points);

        assert!(result.is_ok());
        let deposit_price = result.unwrap();
        // Expected: 100 * (1 + 1) = 200
        let expected = Decimal::from_atomics(200u128, 0).unwrap();
        assert_eq!(deposit_price, expected);
    }

    #[test]
    fn test_get_deposit_price_fractional_exchange_rate() {
        let exchange_rate = Decimal::from_str("1.1234").unwrap(); // 1.1234
        let price_multiplier_basis_points = 250u128; // 2.5%

        let result = get_deposit_price(exchange_rate, price_multiplier_basis_points);

        assert!(result.is_ok());
        let deposit_price = result.unwrap();
        // Expected: 1.1234 * (1 + 0.025) = 1.151485
        let expected = Decimal::from_str("1.151485").unwrap();
        assert_eq!(deposit_price, expected);
    }

    #[test]
    fn test_get_burn_and_refund_messages_success() {
        let env = create_mock_env();
        let max_btc_amount = Uint128::new(1000000); // 1 MAXBTC
        let beneficiary = "treasury".to_string();
        let max_btc_denom = "maxbtc".to_string();
        // Original price = 1.0, New price = 1.05 (5% increase)
        let original_price = Decimal::from_str("1.0").unwrap();
        let new_price = Decimal::from_str("1.05").unwrap();
        // expected burn amount = 1000000 * (1/1.05) = 952380
        let expected_burn_amount_static = Uint128::new(952380);
        // expected return amount = 1000000 - 952380 = 47620
        let expected_return_amount_static = Uint128::new(47620);

        let result = get_burn_and_refund_messages(
            env.clone(),
            max_btc_amount,
            original_price,
            new_price,
            beneficiary.clone(),
            max_btc_denom.clone(),
        );

        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 2);

        // Calculate expected return amount using the same logic as the function
        let price_divergence = original_price.checked_div(new_price).unwrap();
        let expected_burn_amount = Decimal::from_atomics(max_btc_amount, 0)
            .unwrap()
            .checked_mul(price_divergence)
            .unwrap()
            .to_uint_floor();
        let expected_return_amount = max_btc_amount.checked_sub(expected_burn_amount).unwrap();

        // Extract actual send amount from the message
        let (actual_send_amount, actual_recipient) =
            extract_send_amount_and_recipient(&messages[1])
                .expect("Second message should be Bank::Send");

        // Verify the actual message contents match our calculations
        assert_eq!(actual_send_amount, expected_return_amount);
        assert_eq!(actual_recipient, beneficiary);

        assert_eq!(expected_return_amount, expected_return_amount_static);

        // Verify message structure using helper
        let (burn_valid, send_valid) = verify_burn_and_send_messages(
            &messages,
            expected_return_amount,
            &beneficiary,
            &max_btc_denom,
        );
        assert!(burn_valid, "First message should be burn message");
        assert!(send_valid, "Second message should be valid send message");

        // Verify the burn amount calculation by checking what's NOT sent
        let actual_burn_amount = max_btc_amount.checked_sub(actual_send_amount).unwrap();
        assert_eq!(actual_burn_amount, expected_burn_amount);
        assert_eq!(actual_burn_amount, expected_burn_amount_static);
    }

    #[test]
    fn test_get_burn_and_refund_messages_zero_amount() {
        let env = create_mock_env();
        let max_btc_amount = Uint128::zero();
        let beneficiary = "treasury".to_string();
        let max_btc_denom = "maxbtc".to_string();
        let original_price = Decimal::from_str("1.0").unwrap();
        let new_price = Decimal::from_str("1.05").unwrap();
        // expected burn amount = 0
        // expected return amount = 0
        // there is no amount to sell
        let result = get_burn_and_refund_messages(
            env,
            max_btc_amount,
            original_price,
            new_price,
            beneficiary.clone(),
            max_btc_denom.clone(),
        );

        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 2);

        // Extract actual send amount from the message
        let (actual_send_amount, actual_recipient) =
            extract_send_amount_and_recipient(&messages[1])
                .expect("Second message should be Bank::Send");

        // Verify actual amounts are zero
        assert_eq!(actual_send_amount, Uint128::zero());
        assert_eq!(actual_recipient, beneficiary);

        // Verify message structure
        let (burn_valid, send_valid) =
            verify_burn_and_send_messages(&messages, Uint128::zero(), &beneficiary, &max_btc_denom);
        assert!(burn_valid, "First message should be burn message");
        assert!(send_valid, "Second message should be valid send message");

        // Verify burn amount (should also be zero)
        let actual_burn_amount = max_btc_amount.checked_sub(actual_send_amount).unwrap();
        assert_eq!(actual_burn_amount, Uint128::zero());
    }

    #[test]
    fn test_get_burn_and_refund_messages_equal_prices() {
        let env = create_mock_env();
        let max_btc_amount = Uint128::new(1000000);
        let beneficiary = "treasury".to_string();
        let max_btc_denom = "maxbtc".to_string();
        let original_price = Decimal::from_str("1.0").unwrap();
        let new_price = Decimal::from_str("1.0").unwrap();
        // expected burn amount = 0
        // expected return amount = 0
        // because the prices are equal, there was no surcharge in selling
        let result = get_burn_and_refund_messages(
            env,
            max_btc_amount,
            original_price,
            new_price,
            beneficiary.clone(),
            max_btc_denom.clone(),
        );

        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 2);

        // Extract actual send amount from the message
        let (actual_send_amount, actual_recipient) =
            extract_send_amount_and_recipient(&messages[1])
                .expect("Second message should be Bank::Send");

        // With equal prices, nothing should be sent to treasury (all burned)
        assert_eq!(actual_send_amount, Uint128::zero());
        assert_eq!(actual_recipient, beneficiary);

        // Verify message structure
        let (burn_valid, send_valid) =
            verify_burn_and_send_messages(&messages, Uint128::zero(), &beneficiary, &max_btc_denom);
        assert!(burn_valid, "First message should be burn message");
        assert!(send_valid, "Second message should be valid send message");

        // Verify all amount was burned
        let actual_burn_amount = max_btc_amount.checked_sub(actual_send_amount).unwrap();
        assert_eq!(actual_burn_amount, max_btc_amount);
        assert_eq!(actual_burn_amount, Uint128::new(1000000));
    }

    #[test]
    fn test_get_burn_and_refund_messages_large_price_increase() {
        let env = create_mock_env();
        let max_btc_amount = Uint128::new(1000000);
        let beneficiary = "treasury".to_string();
        let max_btc_denom = "maxbtc".to_string();
        let original_price = Decimal::from_str("1.0").unwrap();
        let new_price = Decimal::from_str("2.0").unwrap();
        let expected_burn_amount_static = Uint128::new(500000);
        let expected_return_amount_static = Uint128::new(500000);

        let result = get_burn_and_refund_messages(
            env,
            max_btc_amount,
            original_price,
            new_price,
            beneficiary.clone(),
            max_btc_denom.clone(),
        );

        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 2);

        // Extract actual send amount from the message
        let (actual_send_amount, actual_recipient) =
            extract_send_amount_and_recipient(&messages[1])
                .expect("Second message should be Bank::Send");

        // With 100% price increase, 50% should go to treasury
        assert_eq!(actual_send_amount, expected_return_amount_static);
        assert_eq!(actual_recipient, beneficiary);

        // Verify message structure
        let (burn_valid, send_valid) = verify_burn_and_send_messages(
            &messages,
            Uint128::new(500000),
            &beneficiary,
            &max_btc_denom,
        );
        assert!(burn_valid, "First message should be burn message");
        assert!(send_valid, "Second message should be valid send message");

        // Verify 50% was burned
        let actual_burn_amount = max_btc_amount.checked_sub(actual_send_amount).unwrap();
        assert_eq!(actual_burn_amount, expected_burn_amount_static);
    }

    #[test]
    fn test_get_burn_and_refund_messages_edge_case_amounts() {
        let env = create_mock_env();
        let max_btc_amount = Uint128::new(1); // Very small amount
        let beneficiary = "treasury".to_string();
        let max_btc_denom = "maxbtc".to_string();
        let original_price = Decimal::from_str("1.0").unwrap();
        let new_price = Decimal::from_str("1.01").unwrap(); // 1% increase
        let expected_burn_amount_static = Uint128::new(0);
        let expected_return_amount_static = Uint128::new(1);
        // this is an edge case condiiton where rounding in small-tokens benefits the treasury.
        // since the amount to burn is calculated off floored decimals, the return amount will always have the 
        // extra small token that was lost by flooring.
        let result = get_burn_and_refund_messages(
            env,
            max_btc_amount,
            original_price,
            new_price,
            beneficiary.clone(),
            max_btc_denom.clone(),
        );

        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 2);

        // Extract actual send amount from the message
        let (actual_send_amount, actual_recipient) =
            extract_send_amount_and_recipient(&messages[1])
                .expect("Second message should be Bank::Send");

        // Due to rounding, all should go to treasury (burn amount rounds to 0)
        assert_eq!(actual_send_amount, expected_return_amount_static);
        assert_eq!(actual_recipient, beneficiary);

        // Verify message structure
        let (burn_valid, send_valid) =
            verify_burn_and_send_messages(&messages, Uint128::new(1), &beneficiary, &max_btc_denom);
        assert!(burn_valid, "First message should be burn message");
        assert!(send_valid, "Second message should be valid send message");

        // Verify burn amount (should be 0 due to rounding)
        let actual_burn_amount = max_btc_amount.checked_sub(actual_send_amount).unwrap();
        assert_eq!(actual_burn_amount, expected_burn_amount_static);
    }

}
