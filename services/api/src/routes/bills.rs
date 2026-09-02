use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    providers::{
        dtone::DtOneAdapter,
        BillQuoteRequest, BillServiceType, CreateBillPaymentRequest, DigitalServicesProvider,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/bills/services", get(get_services))
        .route("/bills/billers", get(get_billers))
        .route("/bills/products", get(get_products))
        .route("/bills/validate", post(validate_customer))
        .route("/bills/quote", post(create_bill_quote))
        .route("/bills/pay", post(pay_bill))
        .route("/bills/transactions/:id", get(get_bill_transaction))
}

#[derive(Debug, Deserialize)]
struct CountryQuery {
    country: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BillersQuery {
    country: Option<String>,
    service: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProductsQuery {
    country: Option<String>,
    biller_id: String,
}

#[derive(Debug, Deserialize)]
struct ValidateRequest {
    biller_id: String,
    account_reference: String,
}

async fn get_services(Query(q): Query<CountryQuery>) -> impl IntoResponse {
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    let adapter = DtOneAdapter::new();
    match adapter.get_supported_services(&country).await {
        Ok(services) => (StatusCode::OK, Json(json!({ "country": country.to_uppercase(), "services": services }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn get_billers(Query(q): Query<BillersQuery>) -> impl IntoResponse {
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    let service_type = q.service.as_deref().and_then(BillServiceType::from_str);
    let adapter = DtOneAdapter::new();
    match adapter.get_billers(&country, service_type.as_ref()).await {
        Ok(billers) => (StatusCode::OK, Json(json!({ "billers": billers }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn get_products(Query(q): Query<ProductsQuery>) -> impl IntoResponse {
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    let adapter = DtOneAdapter::new();
    match adapter.get_products(&country, &q.biller_id).await {
        Ok(products) => (StatusCode::OK, Json(json!({ "products": products }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn validate_customer(Json(req): Json<ValidateRequest>) -> impl IntoResponse {
    let adapter = DtOneAdapter::new();
    match adapter.validate_customer(&req.biller_id, &req.account_reference).await {
        Ok(validation) => (StatusCode::OK, Json(json!(validation))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn create_bill_quote(Json(req): Json<BillQuoteRequest>) -> impl IntoResponse {
    let adapter = DtOneAdapter::new();
    match adapter.get_bill_quote(&req).await {
        Ok(quote) => (StatusCode::OK, Json(json!(quote))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn pay_bill(Json(req): Json<CreateBillPaymentRequest>) -> impl IntoResponse {
    let adapter = DtOneAdapter::new();
    match adapter.pay_bill(&req).await {
        Ok(tx) => (StatusCode::OK, Json(json!(tx))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn get_bill_transaction(Path(id): Path<String>) -> impl IntoResponse {
    let adapter = DtOneAdapter::new();
    match adapter.get_bill_status(&id).await {
        Ok(tx) => (StatusCode::OK, Json(json!(tx))),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))),
    }
}
