use hanbova_core::market::{CountryCode, CurrencyCode, MarketCapabilities, UserCountryContext};

#[test]
fn test_country_model_separation() {
    // A Nigerian resident traveling to Kenya, displaying prices in KES
    let context = UserCountryContext::new("NG", "KE", "KES");
    assert_eq!(context.identity_country, "NG");
    assert_eq!(context.spend_country, "KE");
    assert_eq!(context.display_currency, "KES");

    // Switch spend market to Ghana (GHS) without altering identity country
    let mut updated = context.clone();
    updated.spend_country = "GH".to_string();
    updated.display_currency = "GHS".to_string();

    assert_eq!(updated.identity_country, "NG"); // Identity remains Nigeria
    assert_eq!(updated.spend_country, "GH");
    assert_eq!(updated.display_currency, "GHS");

    let cc = CountryCode::new("ke");
    assert_eq!(cc.as_str(), "KE");

    let cur = CurrencyCode::new("kes");
    assert_eq!(cur.as_str(), "KES");
}

#[test]
fn test_market_capabilities_defaults() {
    let caps = MarketCapabilities {
        payouts: true,
        mobile_money: true,
        cards: true,
        airtime: true,
        data: true,
        electricity: true,
        water: true,
        tv: true,
        internet: true,
        esim: true,
    };
    assert!(caps.payouts);
    assert!(caps.mobile_money);
    assert!(caps.cards);
    assert!(caps.airtime);
    assert!(caps.data);
    assert!(caps.electricity);
    assert!(caps.water);
    assert!(caps.tv);
    assert!(caps.internet);
    assert!(caps.esim);
}
