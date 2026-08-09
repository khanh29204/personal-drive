use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;

use crate::app_state::AppState;
use crate::errors::AppError;
use crate::services::auth_service::{verify_token, AuthenticatedUser};

pub struct AuthUser(pub AuthenticatedUser);
pub struct OptionalAuthUser(pub Option<AuthenticatedUser>);

fn extract_token(parts: &Parts, cookie_name: &str) -> Option<String> {
    if let Some(auth_header) = parts.headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = auth_str["Bearer ".len()..].trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }

    let jar = CookieJar::from_headers(&parts.headers);
    if let Some(cookie) = jar.get(cookie_name) {
        let val = cookie.value().trim();
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }

    None
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_token(parts, &state.config.cookie_name)
            .ok_or_else(|| AppError::unauthorized("Yêu cầu đăng nhập"))?;

        let user = verify_token(&state.config, &token).await?;
        Ok(AuthUser(user))
    }
}

impl FromRequestParts<AppState> for OptionalAuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(token) = extract_token(parts, &state.config.cookie_name) {
            if let Ok(user) = verify_token(&state.config, &token).await {
                return Ok(OptionalAuthUser(Some(user)));
            }
        }
        Ok(OptionalAuthUser(None))
    }
}
