use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    providers::{BillQuoteRequest, CreateBillPaymentRequest},
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

use crate::providers::CapabilityStatus;

fn bills_disabled_response() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "code": "provider_unavailable",
            "message": "Bill services are not available in this environment",
            "error": "Bill services are not available in this environment"
        })),
    )
}

async fn get_services(
    State(state): State<AppState>,
    Query(q): Query<CountryQuery>,
) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return bills_disabled_response();
    }
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    match state
        .digital_services_provider
        .get_supported_services(&country)
        .await
    {
        Ok(services) => (
            StatusCode::OK,
            Json(json!({ "country": country.to_uppercase(), "services": services })),
        ),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn get_billers(
    State(state): State<AppState>,
    Query(q): Query<BillersQuery>,
) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return bills_disabled_response();
    }
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    let service_type = q.service.as_deref().and_then(|value| value.parse().ok());
    match state
        .digital_services_provider
        .get_billers(&country, service_type.as_ref())
        .await
    {
        Ok(billers) => (StatusCode::OK, Json(json!({ "billers": billers }))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn get_products(
    State(state): State<AppState>,
    Query(q): Query<ProductsQuery>,
) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return bills_disabled_response();
    }
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    match state
        .digital_services_provider
        .get_products(&country, &q.biller_id)
        .await
    {
        Ok(products) => (StatusCode::OK, Json(json!({ "products": products }))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn validate_customer(
    State(state): State<AppState>,
    Json(req): Json<ValidateRequest>,
) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return bills_disabled_response();
    }
    match state
        .digital_services_provider
        .validate_customer(&req.biller_id, &req.account_reference)
        .await
    {
        Ok(validation) => (StatusCode::OK, Json(json!(validation))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn create_bill_quote(
    State(state): State<AppState>,
    Json(req): Json<BillQuoteRequest>,
) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return bills_disabled_response();
    }
    match state.digital_services_provider.get_bill_quote(&req).await {
        Ok(quote) => (StatusCode::OK, Json(json!(quote))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn pay_bill(
    State(state): State<AppState>,
    Json(req): Json<CreateBillPaymentRequest>,
) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return bills_disabled_response();
    }
    match state.digital_services_provider.pay_bill(&req).await {
        Ok(tx) => (StatusCode::OK, Json(json!(tx))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn get_bill_transaction(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return bills_disabled_response();
    }
    match state.digital_services_provider.get_bill_status(&id).await {
        Ok(tx) => (StatusCode::OK, Json(json!(tx))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}
