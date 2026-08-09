use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use axum_extra::extract::cookie::{Cookie, SameSite};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::errors::AppError;
use crate::services::auth_service::login_via_api;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginBody {
    #[serde(rename = "userName")]
    pub user_name: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
    jar: CookieJar,
    Json(body): Json<LoginBody>,
) -> Result<impl IntoResponse, AppError> {
    if body.user_name.trim().is_empty() || body.password.trim().is_empty() {
        return Err(AppError::bad_request("userName và password không được để trống"));
    }

    let token = login_via_api(&state.config.auth_api_base_url, &body.user_name, &body.password).await?;

    let require_token = query.token.as_deref() == Some("true");

    let mut cookie_builder = Cookie::build((state.config.cookie_name.clone(), token.clone()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(Duration::days(state.config.cookie_max_age_days));

    if let Some(domain) = &state.config.cookie_domain {
        cookie_builder = cookie_builder.domain(domain.clone());
    }

    if state.config.cookie_secure {
        cookie_builder = cookie_builder.secure(true);
    }

    let cookie = cookie_builder.build();
    let updated_jar = jar.add(cookie);

    let res_body = LoginResponse {
        message: "Đăng nhập thành công".to_string(),
        token: if require_token { Some(token) } else { None },
    };

    Ok((updated_jar, Json(res_body)))
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<impl IntoResponse, AppError> {
    let mut cookie_builder = Cookie::build((state.config.cookie_name.clone(), ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(Duration::seconds(0));

    if let Some(domain) = &state.config.cookie_domain {
        cookie_builder = cookie_builder.domain(domain.clone());
    }

    if state.config.cookie_secure {
        cookie_builder = cookie_builder.secure(true);
    }

    let updated_jar = jar.add(cookie_builder.build());

    Ok((updated_jar, Json(serde_json::json!({ "message": "Đã đăng xuất" }))))
}
