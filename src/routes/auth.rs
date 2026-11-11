use axum::{extract::State, http::StatusCode, Json};

use crate::{
    auth::{password, AuthUser},
    db,
    error::{AppError, ErrorResponse},
    extract::ValidatedJson,
    models::user::{LoginRequest, RegisterRequest, TokenResponse, UserResponse},
    state::AppState,
};

/// Register a new account.
#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User created", body = UserResponse),
        (status = 400, description = "Malformed JSON", body = ErrorResponse),
        (status = 409, description = "Username or email already taken", body = ErrorResponse),
        (status = 422, description = "Validation failed", body = ErrorResponse),
    )
)]
pub async fn register(
    State(state): State<AppState>,
    ValidatedJson(input): ValidatedJson<RegisterRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    let username = input.username.trim().to_owned();
    let email = input.email.trim().to_lowercase();
    let hash = password::hash_password(input.password).await?;

    let user = db::users::create(&state.db, &username, &email, &hash).await?;
    tracing::info!(user_id = user.id, "user registered");
    Ok((StatusCode::CREATED, Json(user.into())))
}

/// Exchange username + password for a JWT access token.
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = TokenResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
        (status = 422, description = "Validation failed", body = ErrorResponse),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    ValidatedJson(input): ValidatedJson<LoginRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    let user = db::users::find_by_username(&state.db, input.username.trim()).await?;

    // Always run a hash verification (against a dummy hash for unknown users)
    // so both failure modes take roughly the same time.
    let stored_hash = user.as_ref().map(|u| u.password_hash.clone());
    let valid = password::verify_password(input.password, stored_hash).await?;

    match user {
        Some(user) if valid => {
            let token = state.jwt.issue(user.id, &user.username)?;
            tracing::info!(user_id = user.id, "user logged in");
            Ok(Json(TokenResponse {
                access_token: token,
                token_type: "Bearer".to_owned(),
                expires_in: state.jwt.expiry_seconds(),
            }))
        }
        _ => Err(AppError::unauthorized("Invalid username or password")),
    }
}

/// Return the profile of the authenticated user.
#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "auth",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current user", body = UserResponse),
        (status = 401, description = "Missing or invalid token", body = ErrorResponse),
    )
)]
pub async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<UserResponse>, AppError> {
    let user = db::users::find_by_id(&state.db, auth.id)
        .await?
        .ok_or_else(|| AppError::unauthorized("User no longer exists"))?;
    Ok(Json(user.into()))
}
