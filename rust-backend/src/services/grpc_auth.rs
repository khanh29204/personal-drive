pub mod auth_proto {
    tonic::include_proto!("auth");
}

use auth_proto::auth_service_client::AuthServiceClient;
use auth_proto::VerifyTokenRequest;

pub async fn verify_token_grpc(
    base_url: &str,
    token: &str,
) -> Result<(bool, String, String), tonic::Status> {
    let endpoint = if base_url.starts_with("http://") || base_url.starts_with("https://") {
        base_url.to_string()
    } else {
        format!("http://{}", base_url)
    };

    let mut client = AuthServiceClient::connect(endpoint)
        .await
        .map_err(|e| tonic::Status::unavailable(format!("gRPC connection error: {e}")))?;

    let req = tonic::Request::new(VerifyTokenRequest {
        token: token.to_string(),
    });

    let res = client.verify_token(req).await?.into_inner();
    Ok((res.valid, res.user_id, res.user_name))
}
