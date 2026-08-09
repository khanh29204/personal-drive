use serde::Deserialize;

fn default_port() -> u16 {
    4000
}

fn default_auth_strategy() -> String {
    "local".to_string()
}

fn default_auth_api_base_url() -> String {
    "http://localhost:3000".to_string()
}

fn default_auth_grpc_base_url() -> String {
    "localhost:50051".to_string()
}

fn default_cookie_name() -> String {
    "drive_token".to_string()
}

fn default_cookie_secure() -> bool {
    false
}

fn default_cookie_max_age_days() -> i64 {
    7
}

fn default_r2_upload_url_expires_in() -> u64 {
    300
}

fn default_r2_download_url_expires_in() -> u64 {
    3600
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct AppConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    pub mongodb_uri: String,

    #[serde(default = "default_auth_strategy")]
    pub auth_strategy: String,
    pub jwt_secret: String,
    #[serde(default = "default_auth_api_base_url")]
    pub auth_api_base_url: String,
    #[serde(default = "default_auth_grpc_base_url")]
    pub auth_grpc_base_url: String,

    #[serde(default = "default_cookie_name")]
    pub cookie_name: String,
    #[serde(default)]
    pub cookie_domain: Option<String>,
    #[serde(default = "default_cookie_secure")]
    pub cookie_secure: bool,
    #[serde(default = "default_cookie_max_age_days")]
    pub cookie_max_age_days: i64,
    #[serde(default)]
    pub cors_origin: Option<String>,

    pub r2_account_id: String,
    pub r2_access_key_id: String,
    pub r2_secret_access_key: String,
    pub r2_bucket_name: String,
    #[serde(default = "default_r2_upload_url_expires_in")]
    pub r2_upload_url_expires_in: u64,
    #[serde(default = "default_r2_download_url_expires_in")]
    pub r2_download_url_expires_in: u64,
    #[serde(default)]
    pub r2_public_domain: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();

        let mut map = serde_json::Map::new();

        for (key, val) in std::env::vars() {
            let trimmed = val.trim();
            if trimmed.is_empty() {
                continue;
            }
            let json_val = match key.as_str() {
                "PORT" => {
                    let n = trimmed
                        .parse::<u16>()
                        .map_err(|_| format!("Invalid PORT: {}", trimmed))?;
                    serde_json::Value::from(n)
                }
                "COOKIE_SECURE" => {
                    let b = match trimmed.to_lowercase().as_str() {
                        "true" | "1" => true,
                        "false" | "0" => false,
                        _ => return Err(format!("Invalid COOKIE_SECURE: {}", trimmed)),
                    };
                    serde_json::Value::from(b)
                }
                "COOKIE_MAX_AGE_DAYS" => {
                    let n = trimmed
                        .parse::<i64>()
                        .map_err(|_| format!("Invalid COOKIE_MAX_AGE_DAYS: {}", trimmed))?;
                    serde_json::Value::from(n)
                }
                "R2_UPLOAD_URL_EXPIRES_IN" => {
                    let n = trimmed
                        .parse::<u64>()
                        .map_err(|_| format!("Invalid R2_UPLOAD_URL_EXPIRES_IN: {}", trimmed))?;
                    serde_json::Value::from(n)
                }
                "R2_DOWNLOAD_URL_EXPIRES_IN" => {
                    let n = trimmed
                        .parse::<u64>()
                        .map_err(|_| format!("Invalid R2_DOWNLOAD_URL_EXPIRES_IN: {}", trimmed))?;
                    serde_json::Value::from(n)
                }
                _ => serde_json::Value::String(val),
            };

            map.insert(key, json_val);
        }

        serde_json::from_value::<AppConfig>(serde_json::Value::Object(map))
            .map_err(|e| format!("Lỗi cấu hình môi trường: {}", e))
    }
}
