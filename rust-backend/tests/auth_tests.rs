use jsonwebtoken::{encode, EncodingKey, Header};
use rust_backend::services::auth_service::verify_local;
use serde::Serialize;

#[derive(Serialize)]
struct TestClaims {
    id: String,
    user_name: String,
}

#[test]
fn test_verify_local_jwt_success() {
    let secret = "super_secret_jwt_key_123";
    let claims = TestClaims {
        id: "usr_999".to_string(),
        user_name: "test_user".to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();

    let user = verify_local(&token, secret).expect("Verify token should succeed");
    assert_eq!(user.id, "usr_999");
    assert_eq!(user.user_name, "test_user");
}

#[test]
fn test_verify_local_jwt_invalid_secret() {
    let secret = "super_secret_jwt_key_123";
    let claims = TestClaims {
        id: "usr_999".to_string(),
        user_name: "test_user".to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();

    let res = verify_local(&token, "wrong_secret");
    assert!(res.is_err());
}
