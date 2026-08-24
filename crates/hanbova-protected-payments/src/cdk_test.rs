use cdk::{
    amount::{Amount, SplitTarget},
    nuts::{
        nut10::Conditions, nut11::SigFlag, CurrencyUnit, PaymentMethod, SecretKey,
        SpendingConditions,
    },
    util::unix_time,
    wallet::{ReceiveOptions, SendOptions, Wallet},
};
use cdk_redb::WalletRedbDatabase;
use std::{str::FromStr, sync::Arc};

fn random_seed() -> [u8; 64] {
    let mut seed = [0u8; 64];
    use secp256k1::rand::RngCore;
    secp256k1::rand::rngs::OsRng.fill_bytes(&mut seed);
    seed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scenario_a_bob_claims_with_p2pk() {
        let mint_url = "http://127.0.0.1:3338";

        // 1. Create Alice Wallet
        let alice_dir =
            std::env::temp_dir().join(format!("hanbova_alice_{}", uuid::Uuid::new_v4()));
        let alice_db = Arc::new(WalletRedbDatabase::new(&alice_dir).unwrap());
        let alice_wallet =
            Wallet::new(mint_url, CurrencyUnit::Sat, alice_db, random_seed(), None).unwrap();

        // 2. Fund Alice with 1000 sats via controlled local test backend
        let quote = alice_wallet
            .mint_quote(
                PaymentMethod::from_str("bolt11").unwrap(),
                Some(Amount::from(1000u64)),
                None,
                None,
            )
            .await
            .unwrap();
        alice_wallet
            .mint(&quote.id, SplitTarget::default(), None)
            .await
            .unwrap();

        let alice_bal_after_fund = alice_wallet.total_balance().await.unwrap();
        assert_eq!(
            alice_bal_after_fund,
            Amount::from(1000u64),
            "Alice wallet balance must be exactly 1000 sats after funding"
        );

        // 3. Setup Bob and Alice keys
        let bob_sec = SecretKey::generate();
        let bob_pub = bob_sec.public_key();

        let alice_refund_sec = SecretKey::generate();
        let alice_refund_pub = alice_refund_sec.public_key();

        let locktime = unix_time() + 60;

        let conditions = Conditions::new(
            Some(locktime),
            None,
            Some(vec![alice_refund_pub]),
            None,
            Some(SigFlag::SigInputs),
            None,
        )
        .unwrap();
        let spending_conditions = SpendingConditions::new_p2pk(bob_pub, Some(conditions));

        // 4. Alice sends 100 sats protected
        let send_options = SendOptions {
            conditions: Some(spending_conditions),
            ..Default::default()
        };
        let prepared_send = alice_wallet
            .prepare_send(Amount::from(100u64), send_options)
            .await
            .unwrap();
        let token = prepared_send.confirm(None).await.unwrap();
        let token_str = token.to_string();

        let alice_bal_after_send = alice_wallet.total_balance().await.unwrap();
        // 100 sats sent + 1 sat mint split fee = 899 sats remaining
        assert_eq!(
            alice_bal_after_send,
            Amount::from(899u64),
            "Alice spendable balance must be 899 sats after sending 100 sats (1 sat mint split fee)"
        );

        // 5. Wrong key (Charlie) cannot claim
        let charlie_sec = SecretKey::generate();
        let charlie_dir =
            std::env::temp_dir().join(format!("hanbova_charlie_{}", uuid::Uuid::new_v4()));
        let charlie_db = Arc::new(WalletRedbDatabase::new(&charlie_dir).unwrap());
        let charlie_wallet =
            Wallet::new(mint_url, CurrencyUnit::Sat, charlie_db, random_seed(), None).unwrap();

        let charlie_recv_opts = ReceiveOptions {
            p2pk_signing_keys: vec![charlie_sec],
            ..Default::default()
        };
        let charlie_claim = charlie_wallet.receive(&token_str, charlie_recv_opts).await;
        assert!(
            charlie_claim.is_err(),
            "Third-party key without authorization must be rejected"
        );

        // 6. Alice cannot refund before locktime
        let alice_refund_opts = ReceiveOptions {
            p2pk_signing_keys: vec![alice_refund_sec.clone()],
            ..Default::default()
        };
        let early_refund = alice_wallet.receive(&token_str, alice_refund_opts).await;
        assert!(early_refund.is_err(), "Refund before locktime must fail");

        // 7. Bob claims successfully
        let bob_dir = std::env::temp_dir().join(format!("hanbova_bob_{}", uuid::Uuid::new_v4()));
        let bob_db = Arc::new(WalletRedbDatabase::new(&bob_dir).unwrap());
        let bob_wallet =
            Wallet::new(mint_url, CurrencyUnit::Sat, bob_db, random_seed(), None).unwrap();

        let bob_bal_before = bob_wallet.total_balance().await.unwrap();
        assert_eq!(bob_bal_before, Amount::ZERO, "Bob initial balance must be 0");

        let bob_recv_opts = ReceiveOptions {
            p2pk_signing_keys: vec![bob_sec],
            ..Default::default()
        };
        let bob_received = bob_wallet.receive(&token_str, bob_recv_opts).await.unwrap();
        // 100 sats token received - 1 sat swap fee = 99 sats
        assert_eq!(
            bob_received,
            Amount::from(99u64),
            "Bob received net amount must be 99 sats (after 1 sat swap fee)"
        );

        let bob_bal_after = bob_wallet.total_balance().await.unwrap();
        assert_eq!(
            bob_bal_after,
            Amount::from(99u64),
            "Bob total balance after claim must be exactly 99 sats"
        );

        // 8. Alice refund after Bob claimed must fail (already spent)
        let alice_late_opts = ReceiveOptions {
            p2pk_signing_keys: vec![alice_refund_sec],
            ..Default::default()
        };
        let late_refund = alice_wallet.receive(&token_str, alice_late_opts).await;
        assert!(
            late_refund.is_err(),
            "Refund after recipient claimed must fail"
        );
    }

    #[tokio::test]
    async fn test_scenario_b_alice_refunds_after_locktime() {
        let mint_url = "http://127.0.0.1:3338";

        // 1. Create Alice Wallet
        let alice_dir =
            std::env::temp_dir().join(format!("hanbova_alice_{}", uuid::Uuid::new_v4()));
        let alice_db = Arc::new(WalletRedbDatabase::new(&alice_dir).unwrap());
        let alice_wallet =
            Wallet::new(mint_url, CurrencyUnit::Sat, alice_db, random_seed(), None).unwrap();

        // 2. Fund Alice with 1000 sats via controlled local test backend
        let quote = alice_wallet
            .mint_quote(
                PaymentMethod::from_str("bolt11").unwrap(),
                Some(Amount::from(1000u64)),
                None,
                None,
            )
            .await
            .unwrap();
        alice_wallet
            .mint(&quote.id, SplitTarget::default(), None)
            .await
            .unwrap();

        let alice_bal_start = alice_wallet.total_balance().await.unwrap();
        assert_eq!(alice_bal_start, Amount::from(1000u64));

        // 3. Setup Bob and Alice keys with 2-second short development locktime
        let bob_sec = SecretKey::generate();
        let bob_pub = bob_sec.public_key();

        let alice_refund_sec = SecretKey::generate();
        let alice_refund_pub = alice_refund_sec.public_key();

        let locktime = unix_time() + 2; // 2 seconds

        let conditions = Conditions::new(
            Some(locktime),
            None,
            Some(vec![alice_refund_pub]),
            None,
            Some(SigFlag::SigInputs),
            None,
        )
        .unwrap();
        let spending_conditions = SpendingConditions::new_p2pk(bob_pub, Some(conditions));

        // 4. Alice sends 100 sats protected
        let send_options = SendOptions {
            conditions: Some(spending_conditions),
            ..Default::default()
        };
        let prepared_send = alice_wallet
            .prepare_send(Amount::from(100u64), send_options)
            .await
            .unwrap();
        let token = prepared_send.confirm(None).await.unwrap();
        let token_str = token.to_string();

        let alice_bal_sent = alice_wallet.total_balance().await.unwrap();
        assert_eq!(alice_bal_sent, Amount::from(899u64));

        // 5. Wait for locktime to expire (3 seconds)
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // 6. Alice refunds using her refund private key
        let alice_refund_opts = ReceiveOptions {
            p2pk_signing_keys: vec![alice_refund_sec],
            ..Default::default()
        };
        let refund_received = alice_wallet
            .receive(&token_str, alice_refund_opts)
            .await
            .unwrap();
        assert_eq!(
            refund_received,
            Amount::from(99u64),
            "Alice refund received net amount must be 99 sats (after 1 sat swap fee)"
        );

        let alice_bal_refunded = alice_wallet.total_balance().await.unwrap();
        assert_eq!(
            alice_bal_refunded,
            Amount::from(998u64),
            "Alice balance after 100 sat refund must be 998 sats (899 + 99)"
        );

        // 7. Bob attempts claim after Alice refunded -> must fail
        let bob_dir = std::env::temp_dir().join(format!("hanbova_bob_{}", uuid::Uuid::new_v4()));
        let bob_db = Arc::new(WalletRedbDatabase::new(&bob_dir).unwrap());
        let bob_wallet =
            Wallet::new(mint_url, CurrencyUnit::Sat, bob_db, random_seed(), None).unwrap();

        let bob_recv_opts = ReceiveOptions {
            p2pk_signing_keys: vec![bob_sec],
            ..Default::default()
        };
        let bob_claim = bob_wallet.receive(&token_str, bob_recv_opts).await;
        assert!(bob_claim.is_err(), "Claim after refund must fail");
    }
}
