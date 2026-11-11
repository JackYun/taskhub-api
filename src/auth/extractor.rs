//! `AuthUser` extractor: requires `Authorization: Bearer <token>`.

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::{error::AppError, state::AppState};

/// The authenticated caller, resolved from a valid bearer token.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .ok_or_else(|| AppError::unauthorized("Missing Authorization header"))?
            .to_str()
            .map_err(|_| AppError::unauthorized("Malformed Authorization header"))?;

        let token = header
            .split_once(' ')
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map(|(_, token)| token.trim())
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                AppError::unauthorized("Authorization header must be 'Bearer <token>'")
            })?;

        let claims = state.jwt.verify(token)?;
        let id = claims
            .sub
            .parse::<i64>()
            .map_err(|_| AppError::unauthorized("Invalid or expired token"))?;

        Ok(Self {
            id,
            username: claims.username,
        })
    }
}
