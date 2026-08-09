use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::errors::AppError;
use crate::services::grpc_auth::verify_token_grpc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: String,
    #[serde(rename = "userName")]
    pub user_name: String,
}

#[derive(Debug, Deserialize)]
struct TokenClaims {
    pub id: Option<String>,
    #[serde(rename = "_id")]
    pub _id: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    pub user_name: String,
}

#[derive(Debug, Deserialize)]
struct ApiCheckTokenResponse {
    pub id: Option<String>,
    #[serde(rename = "_id")]
    pub _id: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    pub user_name: String,
}

#[derive(Debug, Serialize)]
struct ApiLoginRequest<'a> {
    #[serde(rename = "userName")]
    pub user_name: &'a str,
    pub password: &'a str,
}

#[derive(Debug, Deserialize)]
struct ApiLoginResponse {
    pub token: String,
}

pub fn verify_local(token: &str, secret: &str) -> Result<AuthenticatedUser, AppError> {
    let key = DecodingKey::from_secret(secret.as_bytes());
    let mut validation = Validation::default();
    validation.validate_exp = false;
    validation.required_spec_claims.clear();

    let token_data = decode::<TokenClaims>(token, &key, &validation)
        .map_err(|_| AppError::unauthorized("Token không hợp lệ"))?;

    let claims = token_data.claims;
    let uid = claims
        .id
        .or(claims._id)
        .or(claims.user_id)
        .ok_or_else(|| AppError::unauthorized("Token format không hợp lệ"))?;

    Ok(AuthenticatedUser {
        id: uid,
        user_name: claims.user_name,
    })
}

pub async fn verify_via_api(base_url: &str, token: &str) -> Result<AuthenticatedUser, AppError> {
    let client = reqwest::Client::new();
    let url = format!("{}/auth/checkToken", base_url.trim_end_matches('/'));

    let res = client
        .post(&url)
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("API call failed: {e}")))?;

    if res.status().as_u16() == 401 {
        return Err(AppError::unauthorized("Token không hợp lệ"));
    }

    if !res.status().is_success() {
        return Err(AppError::unauthorized("Xác thực token thất bại"));
    }

    let data: ApiCheckTokenResponse = res
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Parse auth response error: {e}")))?;

    let uid = data
        .id
        .or(data._id)
        .or(data.user_id)
        .ok_or_else(|| AppError::unauthorized("Token format không hợp lệ"))?;

    Ok(AuthenticatedUser {
        id: uid,
        user_name: data.user_name,
    })
}

pub async fn verify_via_grpc(base_url: &str, token: &str) -> Result<AuthenticatedUser, AppError> {
    let (valid, user_id, user_name) = verify_token_grpc(base_url, token)
        .await
        .map_err(|_| AppError::unauthorized("Xác thực gRPC thất bại"))?;

    if !valid || user_id.is_empty() {
        return Err(AppError::unauthorized("Token không hợp lệ"));
    }

    Ok(AuthenticatedUser {
        id: user_id,
        user_name,
    })
}

pub async fn verify_token(
    config: &AppConfig,
    token: &str,
) -> Result<AuthenticatedUser, AppError> {
    match config.auth_strategy.as_str() {
        "grpc" => verify_via_grpc(&config.auth_grpc_base_url, token).await,
        "api" => verify_via_api(&config.auth_api_base_url, token).await,
        _ => verify_local(token, &config.jwt_secret),
    }
}

pub async fn login_via_api(
    base_url: &str,
    user_name: &str,
    password: &str,
) -> Result<String, AppError> {
    let client = reqwest::Client::new();
    let url = format!("{}/auth/login", base_url.trim_end_matches('/'));

    let req_body = ApiLoginRequest {
        user_name,
        password,
    };

    let res = client
        .post(&url)
        .json(&req_body)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("HTTP login error: {e}")))?;

    if res.status().as_u16() == 401 {
        return Err(AppError::unauthorized("Sai tên đăng nhập hoặc mật khẩu"));
    }

    if !res.status().is_success() {
        return Err(AppError::Internal("Đăng nhập thất bại từ auth service".into()));
    }

    let data: ApiLoginResponse = res
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Parse login response error: {e}")))?;

    Ok(data.token)
}
