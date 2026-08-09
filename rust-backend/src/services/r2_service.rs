use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::{Region, SharedCredentialsProvider};
use aws_sdk_s3::presigning::PresigningConfig;
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

pub fn build_object_key(owner_id: &str, original_name: &str) -> String {
    let safe_name: String = original_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '_' })
        .collect();
    format!("{}/{}-{}", owner_id, Uuid::new_v4(), safe_name)
}

fn encode_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
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

        Self {
            client,
            bucket_name: config.r2_bucket_name.clone(),
            upload_expires_in: config.r2_upload_url_expires_in,
            download_expires_in: config.r2_download_url_expires_in,
            public_domain: config.r2_public_domain.clone(),
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
                return Ok(format!("{}/{}", base, key));
            }
        }

        let disposition = if inline {
            format!("inline; filename=\"{}\"", encode_filename(download_name.unwrap_or("")))
        } else {
            format!("attachment; filename=\"{}\"", encode_filename(download_name.unwrap_or("")))
        };

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
            Err(_) => Ok((false, None)),
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
}
