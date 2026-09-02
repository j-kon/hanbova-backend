use axum::{extract::Path, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use hanbova_core::market::{MarketCapabilities, MarketInfo};
use serde_json::json;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/markets", get(list_markets)).route(
        "/markets/:country/capabilities",
        get(get_market_capabilities),
    )
}

async fn list_markets() -> impl IntoResponse {
    let markets = vec![
        MarketInfo {
            country: "KE".to_string(),
            name: "Kenya".to_string(),
            flag_emoji: "🇰🇪".to_string(),
            currency: "KES".to_string(),
            dial_code: "+254".to_string(),
            environment: "sandbox".to_string(),
            source: "mock".to_string(),
            capabilities: MarketCapabilities {
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
            },
        },
        MarketInfo {
            country: "NG".to_string(),
            name: "Nigeria".to_string(),
            flag_emoji: "🇳🇬".to_string(),
            currency: "NGN".to_string(),
            dial_code: "+234".to_string(),
            environment: "sandbox".to_string(),
            source: "mock".to_string(),
            capabilities: MarketCapabilities {
                payouts: true,
                mobile_money: false,
                cards: true,
                airtime: true,
                data: true,
                electricity: true,
                water: false,
                tv: true,
                internet: true,
                esim: true,
            },
        },
        MarketInfo {
            country: "GH".to_string(),
            name: "Ghana".to_string(),
            flag_emoji: "🇬🇭".to_string(),
            currency: "GHS".to_string(),
            dial_code: "+233".to_string(),
            environment: "sandbox".to_string(),
            source: "mock".to_string(),
            capabilities: MarketCapabilities {
                payouts: true,
                mobile_money: true,
                cards: true,
                airtime: true,
                data: true,
                electricity: true,
                water: true,
                tv: true,
                internet: false,
                esim: true,
            },
        },
        MarketInfo {
            country: "ZA".to_string(),
            name: "South Africa".to_string(),
            flag_emoji: "🇿🇦".to_string(),
            currency: "ZAR".to_string(),
            dial_code: "+27".to_string(),
            environment: "sandbox".to_string(),
            source: "mock".to_string(),
            capabilities: MarketCapabilities {
                payouts: true,
                mobile_money: false,
                cards: true,
                airtime: true,
                data: true,
                electricity: true,
                water: false,
                tv: true,
                internet: true,
                esim: true,
            },
        },
        MarketInfo {
            country: "UG".to_string(),
            name: "Uganda".to_string(),
            flag_emoji: "🇺🇬".to_string(),
            currency: "UGX".to_string(),
            dial_code: "+256".to_string(),
            environment: "sandbox".to_string(),
            source: "mock".to_string(),
            capabilities: MarketCapabilities {
                payouts: true,
                mobile_money: true,
                cards: true,
                airtime: true,
                data: true,
                electricity: true,
                water: true,
                tv: true,
                internet: false,
                esim: true,
            },
        },
        MarketInfo {
            country: "RW".to_string(),
            name: "Rwanda".to_string(),
            flag_emoji: "🇷🇼".to_string(),
            currency: "RWF".to_string(),
            dial_code: "+250".to_string(),
            environment: "sandbox".to_string(),
            source: "mock".to_string(),
            capabilities: MarketCapabilities {
                payouts: true,
                mobile_money: true,
                cards: true,
                airtime: true,
                data: true,
                electricity: true,
                water: true,
                tv: true,
                internet: false,
                esim: true,
            },
        },
    ];

    (StatusCode::OK, Json(markets))
}

async fn get_market_capabilities(Path(country): Path<String>) -> impl IntoResponse {
    let country_upper = country.trim().to_uppercase();
    match country_upper.as_str() {
        "KE" => (
            StatusCode::OK,
            Json(json!({
                "country": "KE",
                "name": "Kenya",
                "currency": "KES",
                "flag_emoji": "🇰🇪",
                "environment": "sandbox",
                "source": "mock",
                "capabilities": {
                    "payouts": true,
                    "mobile_money": true,
                    "cards": true,
                    "airtime": true,
                    "data": true,
                    "electricity": true,
                    "water": true,
                    "tv": true,
                    "internet": true,
                    "esim": true
                }
            })),
        ),
        "NG" => (
            StatusCode::OK,
            Json(json!({
                "country": "NG",
                "name": "Nigeria",
                "currency": "NGN",
                "flag_emoji": "🇳🇬",
                "environment": "sandbox",
                "source": "mock",
                "capabilities": {
                    "payouts": true,
                    "mobile_money": false,
                    "cards": true,
                    "airtime": true,
                    "data": true,
                    "electricity": true,
                    "water": false,
                    "tv": true,
                    "internet": true,
                    "esim": true
                }
            })),
        ),
        "GH" => (
            StatusCode::OK,
            Json(json!({
                "country": "GH",
                "name": "Ghana",
                "currency": "GHS",
                "flag_emoji": "🇬🇭",
                "environment": "sandbox",
                "source": "mock",
                "capabilities": {
                    "payouts": true,
                    "mobile_money": true,
                    "cards": true,
                    "airtime": true,
                    "data": true,
                    "electricity": true,
                    "water": true,
                    "tv": true,
                    "internet": false,
                    "esim": true
                }
            })),
        ),
        "ZA" => (
            StatusCode::OK,
            Json(json!({
                "country": "ZA",
                "name": "South Africa",
                "currency": "ZAR",
                "flag_emoji": "🇿🇦",
                "environment": "sandbox",
                "source": "mock",
                "capabilities": {
                    "payouts": true,
                    "mobile_money": false,
                    "cards": true,
                    "airtime": true,
                    "data": true,
                    "electricity": true,
                    "water": false,
                    "tv": true,
                    "internet": true,
                    "esim": true
                }
            })),
        ),
        "UG" => (
            StatusCode::OK,
            Json(json!({
                "country": "UG",
                "name": "Uganda",
                "currency": "UGX",
                "flag_emoji": "🇺🇬",
                "environment": "sandbox",
                "source": "mock",
                "capabilities": {
                    "payouts": true,
                    "mobile_money": true,
                    "cards": true,
                    "airtime": true,
                    "data": true,
                    "electricity": true,
                    "water": true,
                    "tv": true,
                    "internet": false,
                    "esim": true
                }
            })),
        ),
        "RW" => (
            StatusCode::OK,
            Json(json!({
                "country": "RW",
                "name": "Rwanda",
                "currency": "RWF",
                "flag_emoji": "🇷🇼",
                "environment": "sandbox",
                "source": "mock",
                "capabilities": {
                    "payouts": true,
                    "mobile_money": true,
                    "cards": true,
                    "airtime": true,
                    "data": true,
                    "electricity": true,
                    "water": true,
                    "tv": true,
                    "internet": false,
                    "esim": true
                }
            })),
        ),
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "unsupported_country",
                "message": format!("Market {} is not supported", country_upper)
            })),
        ),
    }
}
