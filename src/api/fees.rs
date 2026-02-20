//! GET /api/fees endpoint — fee structure and fee calculation API

use crate::cache::cache::Cache;
use crate::cache::keys::fee::{fees_calculated, fees_comparison, FEES_ALL};
use crate::cache::RedisCache;
use crate::database::error::DatabaseError;
use crate::services::fee_calculation::FeeCalculationService;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

const TTL_FULL_SECS: u64 = 300;
const TTL_CALCULATED_SECS: u64 = 60;
const SUPPORTED_TYPES: [&str; 3] = ["onramp", "offramp", "bill_payment"];
const SUPPORTED_PROVIDERS: [&str; 3] = ["flutterwave", "paystack", "mpesa"];

#[derive(Clone)]
pub struct FeesState {
    pub fee_service: Arc<FeeCalculationService>,
    pub cache: Option<RedisCache>,
}

#[derive(Debug, Deserialize)]
pub struct FeesQueryParams {
    pub amount: Option<String>,
    #[serde(rename = "type")]
    pub transaction_type: Option<String>,
    pub provider: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum FeesResponse {
    Full(FullFeeStructureResponse),
    Calculated(CalculatedFeesResponse),
    Comparison(FeeComparisonResponse),
}

#[derive(Debug, Serialize)]
pub struct FullFeeStructureResponse {
    pub fee_structure: serde_json::Value,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct CalculatedFeesResponse {
    pub amount: f64,
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub provider: String,
    pub breakdown: FeeBreakdownResponse,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct FeeBreakdownResponse {
    pub platform_fee_ngn: f64,
    pub provider_fee_ngn: f64,
    pub total_fee_ngn: f64,
    pub amount_after_fees_ngn: f64,
    pub platform_fee_pct: f64,
    pub provider_fee_pct: f64,
}

#[derive(Debug, Serialize)]
pub struct FeeComparisonResponse {
    pub amount: f64,
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub comparison: Vec<ProviderFeeEntry>,
    pub cheapest_provider: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderFeeEntry {
    pub provider: String,
    pub platform_fee_ngn: f64,
    pub provider_fee_ngn: f64,
    pub total_fee_ngn: f64,
    pub amount_after_fees_ngn: f64,
}

#[derive(Debug, Serialize)]
pub struct FeesErrorResponse {
    pub error: FeesErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct FeesErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_providers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u64>,
}

pub async fn get_fees(
    State(state): State<FeesState>,
    Query(params): Query<FeesQueryParams>,
) -> Response {
    match validate_params(&params) {
        Err(r) => return r,
        Ok(()) => {}
    }

    // 1. Parse params
    let amount = params.amount.as_deref();
    let tx_type = params.transaction_type.as_deref();
    let provider = params.provider.as_deref();

    // 2. Cache key and TTL
    let (cache_key, ttl_secs) = match (amount, tx_type, provider) {
        (None, None, None) => (FEES_ALL.to_string(), TTL_FULL_SECS),
        (Some(amt), Some(ty), Some(pr)) => (fees_calculated(ty, pr, amt), TTL_CALCULATED_SECS),
        (Some(amt), Some(ty), None) => (fees_comparison(ty, amt), TTL_CALCULATED_SECS),
        _ => (String::new(), 0),
    };

    if !cache_key.is_empty() {
        if let Some(ref cache) = state.cache {
            let cached: Result<Option<serde_json::Value>, _> = cache.get(&cache_key).await;
            if let Ok(Some(cached)) = cached {
                debug!("Fees cache hit: {}", cache_key);
                return (StatusCode::OK, Json(cached)).into_response();
            }
        }
    }

    let result = match (amount, tx_type, provider) {
        (None, None, None) => build_full_structure(&state).await,
        (Some(amt), Some(ty), Some(pr)) => {
            let amt_bd = parse_amount(amt);
            build_calculated(&state, amt_bd, ty, pr).await
        }
        (Some(amt), Some(ty), None) => {
            let amt_bd = parse_amount(amt);
            build_comparison(&state, amt_bd, ty).await
        }
        _ => Err(FeesError::Validation("Invalid params".to_string())),
    };

    match result {
        Ok(resp) => {
            let json = serde_json::to_value(&resp).unwrap_or_default();
            if !cache_key.is_empty() && ttl_secs > 0 {
                if let Some(ref cache) = state.cache {
                    let _ = cache
                        .set(&cache_key, &json, Some(Duration::from_secs(ttl_secs)))
                        .await;
                }
            }
            (StatusCode::OK, Json(json)).into_response()
        }
        Err(e) => error_response(e),
    }
}

fn validate_params(params: &FeesQueryParams) -> Result<(), Response> {
    if params.amount.is_some() && params.transaction_type.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(FeesErrorResponse {
                error: FeesErrorDetail {
                    code: "MISSING_TYPE".to_string(),
                    message: "Query param 'type' is required when 'amount' is provided.".to_string(),
                    supported_types: Some(SUPPORTED_TYPES.iter().map(|s| (*s).to_string()).collect()),
                    supported_providers: None,
                    retry_after: None,
                },
            }),
        )
            .into_response());
    }

    if let Some(ref ty) = params.transaction_type {
        if !SUPPORTED_TYPES.contains(&ty.to_lowercase().as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(FeesErrorResponse {
                    error: FeesErrorDetail {
                        code: "INVALID_TYPE".to_string(),
                        message: format!("Transaction type '{}' is not supported.", ty),
                        supported_types: Some(SUPPORTED_TYPES.iter().map(|s| (*s).to_string()).collect()),
                        supported_providers: None,
                        retry_after: None,
                    },
                }),
            )
                .into_response());
        }
    }

    if let Some(ref pr) = params.provider {
        if !SUPPORTED_PROVIDERS.contains(&pr.to_lowercase().as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(FeesErrorResponse {
                    error: FeesErrorDetail {
                        code: "INVALID_PROVIDER".to_string(),
                        message: format!("Provider '{}' is not supported.", pr),
                        supported_types: None,
                        supported_providers: Some(SUPPORTED_PROVIDERS.iter().map(|s| (*s).to_string()).collect()),
                        retry_after: None,
                    },
                }),
            )
                .into_response());
        }
    }

    if let Some(ref amt) = params.amount {
        if let Ok(a) = BigDecimal::from_str(amt) {
            if a <= BigDecimal::from(0) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(FeesErrorResponse {
                        error: FeesErrorDetail {
                            code: "INVALID_AMOUNT".to_string(),
                            message: "Amount must be a positive number greater than 0.".to_string(),
                            supported_types: None,
                            supported_providers: None,
                            retry_after: None,
                        },
                    }),
                )
                    .into_response());
            }
        } else {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(FeesErrorResponse {
                    error: FeesErrorDetail {
                        code: "INVALID_AMOUNT".to_string(),
                        message: "Amount must be a positive number greater than 0.".to_string(),
                        supported_types: None,
                        supported_providers: None,
                        retry_after: None,
                    },
                }),
            )
                .into_response());
        }
    }

    Ok(())
}

enum FeesError {
    Validation(String),
    Database(DatabaseError),
}

fn parse_amount(s: &str) -> BigDecimal {
    BigDecimal::from_str(s).unwrap_or_else(|_| BigDecimal::from(0))
}

fn bd_to_f64(b: &BigDecimal) -> f64 {
    b.to_string().parse().unwrap_or(0.0)
}

async fn build_full_structure(state: &FeesState) -> Result<FeesResponse, FeesError> {
    let structure = load_full_structure(state).await.map_err(FeesError::Database)?;
    Ok(FeesResponse::Full(FullFeeStructureResponse {
        fee_structure: structure,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }))
}

async fn load_full_structure(state: &FeesState) -> Result<serde_json::Value, DatabaseError> {
    let mut result = serde_json::Map::new();
    for ty in &SUPPORTED_TYPES {
        let mut type_obj = serde_json::Map::new();
        let mut providers_obj = serde_json::Map::new();

        for pr in &SUPPORTED_PROVIDERS {
            let breakdown = state.fee_service.calculate_fees(
                ty,
                BigDecimal::from_str("10000").unwrap(),
                Some(pr),
                Some("card"),
            ).await?;

            let fee_pct = breakdown.provider.as_ref().map(|p| bd_to_f64(&p.percent)).unwrap_or(0.0);
            let flat = breakdown.provider.as_ref().map(|p| bd_to_f64(&p.flat)).unwrap_or(0.0);

            providers_obj.insert(
                pr.to_string(),
                serde_json::json!({
                    "fee_pct": fee_pct,
                    "flat_fee_ngn": flat
                }),
            );
        }

        let sample = state.fee_service.calculate_fees(
            ty,
            BigDecimal::from_str("10000").unwrap(),
            Some("flutterwave"),
            Some("card"),
        ).await?;

        type_obj.insert("platform_fee_pct".to_string(), serde_json::json!(bd_to_f64(&sample.platform.percent)));
        type_obj.insert("min_fee_ngn".to_string(), serde_json::json!(50));
        type_obj.insert("max_fee_ngn".to_string(), serde_json::json!(10000));
        type_obj.insert("providers".to_string(), serde_json::Value::Object(providers_obj));
        result.insert((*ty).to_string(), serde_json::Value::Object(type_obj));
    }

    Ok(serde_json::Value::Object(result))
}

async fn build_calculated(
    state: &FeesState,
    amount: BigDecimal,
    tx_type: &str,
    provider: &str,
) -> Result<FeesResponse, FeesError> {
    let breakdown = state
        .fee_service
        .calculate_fees(tx_type, amount.clone(), Some(provider), Some("card"))
        .await
        .map_err(FeesError::Database)?;

    let platform_fee = bd_to_f64(&breakdown.platform.calculated);
    let provider_fee = breakdown
        .provider
        .as_ref()
        .map(|p| bd_to_f64(&p.calculated))
        .unwrap_or(0.0);
    let total = bd_to_f64(&breakdown.total);
    let net = bd_to_f64(&breakdown.net_amount);
    let platform_pct = bd_to_f64(&breakdown.platform.percent);
    let provider_pct = breakdown
        .provider
        .as_ref()
        .map(|p| bd_to_f64(&p.percent))
        .unwrap_or(0.0);

    Ok(FeesResponse::Calculated(CalculatedFeesResponse {
        amount: bd_to_f64(&amount),
        transaction_type: tx_type.to_string(),
        provider: provider.to_string(),
        breakdown: FeeBreakdownResponse {
            platform_fee_ngn: platform_fee,
            provider_fee_ngn: provider_fee,
            total_fee_ngn: total,
            amount_after_fees_ngn: net,
            platform_fee_pct: platform_pct,
            provider_fee_pct: provider_pct,
        },
        timestamp: chrono::Utc::now().to_rfc3339(),
    }))
}

async fn build_comparison(
    state: &FeesState,
    amount: BigDecimal,
    tx_type: &str,
) -> Result<FeesResponse, FeesError> {
    let mut entries = Vec::new();
    let mut cheapest_provider = "flutterwave".to_string();
    let mut cheapest_total = f64::MAX;

    for pr in &SUPPORTED_PROVIDERS {
        let breakdown = state
            .fee_service
            .calculate_fees(tx_type, amount.clone(), Some(pr), Some("card"))
            .await
            .map_err(FeesError::Database)?;

        let platform_fee = bd_to_f64(&breakdown.platform.calculated);
        let provider_fee = breakdown
            .provider
            .as_ref()
            .map(|p| bd_to_f64(&p.calculated))
            .unwrap_or(0.0);
        let total = bd_to_f64(&breakdown.total);
        let net = bd_to_f64(&breakdown.net_amount);

        if total < cheapest_total {
            cheapest_total = total;
            cheapest_provider = pr.to_string();
        }

        entries.push(ProviderFeeEntry {
            provider: pr.to_string(),
            platform_fee_ngn: platform_fee,
            provider_fee_ngn: provider_fee,
            total_fee_ngn: total,
            amount_after_fees_ngn: net,
        });
    }

    Ok(FeesResponse::Comparison(FeeComparisonResponse {
        amount: bd_to_f64(&amount),
        transaction_type: tx_type.to_string(),
        comparison: entries,
        cheapest_provider,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }))
}

fn error_response(err: FeesError) -> Response {
    match err {
        FeesError::Validation(msg) => (
            StatusCode::BAD_REQUEST,
            Json(FeesErrorResponse {
                error: FeesErrorDetail {
                    code: "VALIDATION_ERROR".to_string(),
                    message: msg,
                    supported_types: None,
                    supported_providers: None,
                    retry_after: None,
                },
            }),
        )
            .into_response(),

        FeesError::Database(_) => {
            info!("Fee service unavailable");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(FeesErrorResponse {
                    error: FeesErrorDetail {
                        code: "FEE_SERVICE_UNAVAILABLE".to_string(),
                        message: "Fee service is temporarily unavailable. Please try again.".to_string(),
                        supported_types: None,
                        supported_providers: None,
                        retry_after: Some(30),
                    },
                }),
            )
                .into_response()
        }
    }
}
