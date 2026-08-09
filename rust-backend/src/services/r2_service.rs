use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::{Region, SharedCredentialsProvider};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::types::{CorsConfiguration, CorsRule};
use aws_sdk_s3::Client as S3Client;
use std::time::Duration;
use uuid::Uuid;

use crate::config::AppConfig;

#[derive(Clone)]
pub struct R2Service {
    client: S3Client,
    bucket_name: String,
    upload_expires_in: u64,
    download_expires_in: u64,
    public_domain: Option<String>,
}

/// Sinh object key cho R2. Chỉ giữ lại ASCII alphanumeric và `._-`, mọi ký tự
/// khác (dấu tiếng Việt, CJK, khoảng trắng) đổi thành `_` — giống bản Node
/// `replace(/[^a-zA-Z0-9._-]/g, '_')`. Dùng `is_ascii_alphanumeric` thay vì
/// `is_alphanumeric`: `is_alphanumeric` cho `true` với 'á', '简', khiến key mang
/// ký tự non-ASCII và phải percent-encode ở mọi chỗ dùng lại key sau này.
pub fn build_object_key(owner_id: &str, original_name: &str) -> String {
    let safe_name: String = original_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}/{}-{}", owner_id, Uuid::new_v4(), safe_name)
}

/// Percent-encode theo từng byte UTF-8 (RFC 3986 unreserved set).
///
/// Bản cũ map trên `char` rồi `c as u8`, tức truncate scalar value xuống 1 byte:
/// 'ế' (U+1EBF) thành 0xBF, mất 2 byte đầu → tên file tải về thành rác. Với
/// chuỗi ASCII thuần kết quả không đổi.
fn percent_encode_bytes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.as_bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

/// Chuỗi an toàn cho tham số `filename=""` của Content-Disposition: bỏ ký tự
/// điều khiển, `"` và `\` để không phá cấu trúc header. Phần Unicode do
/// `filename*=UTF-8''` phụ trách nên ở đây chỉ cần thay bằng `_`.
fn sanitize_ascii_filename(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() && c != '"' && c != '\\' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if cleaned.trim().is_empty() {
        "download".to_string()
    } else {
        cleaned
    }
}

/// Percent-encode từng segment của object key để ghép vào URL public domain.
/// Giữ nguyên `/` vì đó là dấu phân cách path.
fn encode_key_for_url(key: &str) -> String {
    key.split('/')
        .map(percent_encode_bytes)
        .collect::<Vec<_>>()
        .join("/")
}

/// Dựng Content-Disposition theo RFC 6266: `filename` để client cũ đọc được,
/// `filename*` mang tên Unicode đầy đủ. Client hiện đại ưu tiên `filename*`.
fn build_content_disposition(name: &str, inline: bool) -> String {
    let disposition_type = if inline { "inline" } else { "attachment" };
    format!(
        "{}; filename=\"{}\"; filename*=UTF-8''{}",
        disposition_type,
        sanitize_ascii_filename(name),
        percent_encode_bytes(name)
    )
}


impl R2Service {
    pub async fn new(config: &AppConfig) -> Self {
        let endpoint_url = format!("https://{}.r2.cloudflarestorage.com", config.r2_account_id);
        let credentials = Credentials::new(
            &config.r2_access_key_id,
            &config.r2_secret_access_key,
            None,
            None,
            "static",
        );

        let s3_config = aws_sdk_s3::config::Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("auto"))
            .credentials_provider(SharedCredentialsProvider::new(credentials))
            .endpoint_url(&endpoint_url)
            .force_path_style(true)
            .build();

        let client = S3Client::from_conf(s3_config);

        let service = Self {
            client,
            bucket_name: config.r2_bucket_name.clone(),
            upload_expires_in: config.r2_upload_url_expires_in,
            download_expires_in: config.r2_download_url_expires_in,
            public_domain: config.r2_public_domain.clone(),
        };

        service.configure_bucket_cors().await;

        service
    }

    pub async fn configure_bucket_cors(&self) {
        let cors_rule = match CorsRule::builder()
            .allowed_origins("*")
            .allowed_methods("GET")
            .allowed_methods("PUT")
            .allowed_methods("POST")
            .allowed_methods("DELETE")
            .allowed_methods("HEAD")
            .allowed_headers("*")
            .max_age_seconds(3600)
            .build()
        {
            Ok(rule) => rule,
            Err(e) => {
                eprintln!("⚠️ Cảnh báo tạo CorsRule: {}", e);
                return;
            }
        };

        let cors_config = match CorsConfiguration::builder().cors_rules(cors_rule).build() {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("⚠️ Cảnh báo tạo CorsConfiguration: {}", e);
                return;
            }
        };

        match self
            .client
            .put_bucket_cors()
            .bucket(&self.bucket_name)
            .cors_configuration(cors_config)
            .send()
            .await
        {
            Ok(_) => println!("✅ Đã tự động cấu hình R2 Bucket CORS cho '{}'", self.bucket_name),
            Err(e) => eprintln!("⚠️ Cảnh báo gửi put_bucket_cors tới R2: {}", e),
        }
    }

    pub async fn create_upload_url(&self, key: &str, mime_type: &str) -> Result<String, String> {
        let expires_in = Duration::from_secs(self.upload_expires_in);
        let presign_config = PresigningConfig::expires_in(expires_in)
            .map_err(|e| format!("Presigning config error: {e}"))?;

        let presigned_req = self
            .client
            .put_object()
            .bucket(&self.bucket_name)
            .key(key)
            .content_type(mime_type)
            .presigned(presign_config)
            .await
            .map_err(|e| format!("Presign PUT error: {e}"))?;

        Ok(presigned_req.uri().to_string())
    }

    pub async fn create_download_url(
        &self,
        key: &str,
        download_name: Option<&str>,
        inline: bool,
    ) -> Result<String, String> {
        if inline {
            if let Some(domain) = &self.public_domain {
                let base = domain.trim_end_matches('/');
                return Ok(format!("{}/{}", base, encode_key_for_url(key)));
            }
        }

        let disposition = build_content_disposition(download_name.unwrap_or(""), inline);

        let expires_in = Duration::from_secs(self.download_expires_in);
        let presign_config = PresigningConfig::expires_in(expires_in)
            .map_err(|e| format!("Presigning config error: {e}"))?;

        let mut req = self
            .client
            .get_object()
            .bucket(&self.bucket_name)
            .key(key);

        if download_name.is_some() {
            req = req.response_content_disposition(disposition);
        }

        let presigned_req = req
            .presigned(presign_config)
            .await
            .map_err(|e| format!("Presign GET error: {e}"))?;

        Ok(presigned_req.uri().to_string())
    }

    /// Trả về `(tồn tại, size)`. Chỉ coi là "không tồn tại" khi R2 thực sự báo
    /// 404/NotFound; lỗi mạng, credential hỏng hay 5xx được trả về dạng `Err`.
    ///
    /// Trước đây mọi `Err` đều bị nuốt thành `(false, None)`, nên một lần
    /// timeout tới R2 cũng khiến `complete_upload` đánh dấu file `failed` dù
    /// file đã nằm nguyên trên bucket.
    pub async fn get_object_meta(&self, key: &str) -> Result<(bool, Option<i64>), String> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket_name)
            .key(key)
            .send()
            .await
        {
            Ok(output) => Ok((true, output.content_length)),
            Err(err) => {
                // R2 trả 404 cho HEAD mà không kèm body, nên phải xét cả HTTP
                // status thô lẫn biến thể NotFound mà SDK map được.
                let is_404 = err
                    .raw_response()
                    .map(|raw| raw.status().as_u16() == 404)
                    .unwrap_or(false);

                if is_404 {
                    return Ok((false, None));
                }

                let service_err = err.into_service_error();
                if service_err.is_not_found() {
                    return Ok((false, None));
                }

                Err(format!("Head object error: {service_err}"))
            }
        }
    }

    pub async fn delete_object(&self, key: &str) -> Result<(), String> {
        self.client
            .delete_object()
            .bucket(&self.bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|e| format!("Delete object error: {e}"))?;
        Ok(())
    }

    pub async fn list_all_objects(&self) -> Result<Vec<(String, i64)>, String> {
        let mut objects = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut req = self.client.list_objects_v2().bucket(&self.bucket_name);
            if let Some(token) = &continuation_token {
                req = req.continuation_token(token);
            }

            let resp = req.send().await.map_err(|e| format!("List objects error: {e}"))?;

            if let Some(contents) = resp.contents {
                for obj in contents {
                    if let Some(key) = obj.key {
                        let size = obj.size.unwrap_or(0);
                        objects.push((key, size));
                    }
                }
            }

            if resp.is_truncated == Some(true) {
                continuation_token = resp.next_continuation_token;
            } else {
                break;
            }
        }

        Ok(objects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_dung_byte_utf8() {
        // 'á' = C3 A1, không phải E1 (kết quả của `c as u8` trước đây)
        assert_eq!(percent_encode_bytes("Báo cáo.pdf"), "B%C3%A1o%20c%C3%A1o.pdf");
        assert_eq!(percent_encode_bytes("résumé"), "r%C3%A9sum%C3%A9");
        assert_eq!(percent_encode_bytes("简历"), "%E7%AE%80%E5%8E%86");
    }

    #[test]
    fn percent_encode_giu_nguyen_ascii_an_toan() {
        assert_eq!(percent_encode_bytes("normal-file_v2.txt"), "normal-file_v2.txt");
        assert_eq!(percent_encode_bytes("a~b"), "a~b");
        assert_eq!(percent_encode_bytes("a b"), "a%20b");
    }

    #[test]
    fn percent_encode_khop_voi_encodeuricomponent() {
        // encodeURIComponent('Báo cáo tài chính.pdf') trong bản Node
        assert_eq!(
            percent_encode_bytes("Báo cáo tài chính.pdf"),
            "B%C3%A1o%20c%C3%A1o%20t%C3%A0i%20ch%C3%ADnh.pdf"
        );
    }

    #[test]
    fn object_key_chi_chua_ascii() {
        let key = build_object_key("user123", "Báo cáo tài chính.pdf");
        assert!(key.is_ascii(), "object key phải là ASCII thuần: {key}");
        assert!(key.ends_with("-B_o_c_o_t_i_ch_nh.pdf"), "key thực tế: {key}");

        let cjk = build_object_key("user123", "简历.pdf");
        assert!(cjk.is_ascii());
        assert!(cjk.ends_with("-__.pdf"), "key thực tế: {cjk}");
    }

    #[test]
    fn object_key_moi_lan_goi_moi_khac_nhau() {
        let a = build_object_key("user1", "a.txt");
        let b = build_object_key("user1", "a.txt");
        assert_ne!(a, b, "phải có UUID nên không được trùng");
        assert!(a.starts_with("user1/"));
    }

    #[test]
    fn content_disposition_co_ca_filename_va_filename_star() {
        let d = build_content_disposition("Báo cáo.pdf", false);
        assert!(d.starts_with("attachment; "), "{d}");
        // filename thuần ASCII cho client cũ
        assert!(d.contains("filename=\"B_o c_o.pdf\""), "{d}");
        // filename* mang UTF-8 đầy đủ
        assert!(d.contains("filename*=UTF-8''B%C3%A1o%20c%C3%A1o.pdf"), "{d}");
    }

    #[test]
    fn content_disposition_inline_dung_dung_loai() {
        let d = build_content_disposition("a.png", true);
        assert!(d.starts_with("inline; "), "{d}");
    }

    #[test]
    fn content_disposition_khong_the_bi_pha_boi_dau_ngoac() {
        let d = build_content_disposition("evil\"; x=\"y.txt", false);
        // Dấu " trong tên bị thay bằng _ nên header vẫn còn đúng 3 phần
        assert_eq!(d.matches('"').count(), 2, "{d}");
    }

    #[test]
    fn content_disposition_ten_rong_co_fallback() {
        let d = build_content_disposition("", false);
        assert!(d.contains("filename=\"download\""), "{d}");
    }

    #[test]
    fn url_key_giu_dau_gach_cheo() {
        assert_eq!(
            encode_key_for_url("user123/uuid-file name.pdf"),
            "user123/uuid-file%20name.pdf"
        );
        // Key cũ trong DB có thể còn ký tự Unicode, phải encode được
        assert_eq!(
            encode_key_for_url("user123/uuid-Báo_cáo.pdf"),
            "user123/uuid-B%C3%A1o_c%C3%A1o.pdf"
        );
    }

    #[test]
    fn url_key_ascii_khong_bi_doi() {
        let key = "user123/2f8a-normal_file-v2.txt";
        assert_eq!(encode_key_for_url(key), key);
    }
}
