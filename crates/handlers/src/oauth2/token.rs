// Copyright 2021-2023 The Matrix.org Foundation C.I.C.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use axum::{extract::State, response::IntoResponse, Json};
use chrono::{DateTime, Duration, Utc};
use headers::{CacheControl, HeaderMap, HeaderMapExt, Pragma};
use hyper::StatusCode;
use mas_axum_utils::{
    client_authorization::{ClientAuthorization, CredentialsVerificationError},
    http_client_factory::HttpClientFactory,
};
use mas_data_model::{AuthorizationGrantStage, Client};
use mas_keystore::{Encrypter, Keystore};
use mas_router::UrlBuilder;
use mas_storage::{
    oauth2::{
        OAuth2AccessTokenRepository, OAuth2AuthorizationGrantRepository,
        OAuth2RefreshTokenRepository, OAuth2SessionRepository,
    },
    user::BrowserSessionRepository,
    BoxClock, BoxRepository, BoxRng, Clock,
};
use oauth2_types::{
    errors::{ClientError, ClientErrorCode},
    pkce::CodeChallengeError,
    requests::{
        AccessTokenRequest, AccessTokenResponse, AuthorizationCodeGrant, RefreshTokenGrant,
    },
    scope,
};
use serde::Serialize;
use serde_with::{serde_as, skip_serializing_none};
use thiserror::Error;
use tracing::debug;
use url::Url;

use super::{generate_id_token, generate_token_pair};
use crate::impl_from_error_for_route;

#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Debug)]
struct CustomClaims {
    #[serde(rename = "iss")]
    issuer: Url,
    #[serde(rename = "sub")]
    subject: String,
    #[serde(rename = "aud")]
    audiences: Vec<String>,
    nonce: Option<String>,
    #[serde_as(as = "Option<serde_with::TimestampSeconds>")]
    auth_time: Option<DateTime<Utc>>,
    at_hash: String,
    c_hash: String,
}

#[derive(Debug, Error)]
pub(crate) enum RouteError {
    #[error(transparent)]
    Internal(Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("bad request")]
    BadRequest,

    #[error("pkce verification failed")]
    PkceVerification(#[from] CodeChallengeError),

    #[error("client not found")]
    ClientNotFound,

    #[error("client not allowed")]
    ClientNotAllowed,

    #[error("could not verify client credentials")]
    ClientCredentialsVerification(#[from] CredentialsVerificationError),

    #[error("grant not found")]
    GrantNotFound,

    #[error("invalid grant")]
    InvalidGrant,

    #[error("unsupported grant type")]
    UnsupportedGrantType,

    #[error("unauthorized client")]
    UnauthorizedClient,

    #[error("failed to load browser session")]
    NoSuchBrowserSession,

    #[error("failed to load oauth session")]
    NoSuchOAuthSession,
}

impl IntoResponse for RouteError {
    fn into_response(self) -> axum::response::Response {
        sentry::capture_error(&self);
        match self {
            Self::Internal(_) | Self::NoSuchBrowserSession | Self::NoSuchOAuthSession => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ClientError::from(ClientErrorCode::ServerError)),
            ),
            Self::BadRequest => (
                StatusCode::BAD_REQUEST,
                Json(ClientError::from(ClientErrorCode::InvalidRequest)),
            ),
            Self::PkceVerification(err) => (
                StatusCode::BAD_REQUEST,
                Json(
                    ClientError::from(ClientErrorCode::InvalidGrant)
                        .with_description(format!("PKCE verification failed: {err}")),
                ),
            ),
            Self::ClientNotFound | Self::ClientCredentialsVerification(_) => (
                StatusCode::UNAUTHORIZED,
                Json(ClientError::from(ClientErrorCode::InvalidClient)),
            ),
            Self::ClientNotAllowed | Self::UnauthorizedClient => (
                StatusCode::UNAUTHORIZED,
                Json(ClientError::from(ClientErrorCode::UnauthorizedClient)),
            ),
            Self::InvalidGrant | Self::GrantNotFound => (
                StatusCode::BAD_REQUEST,
                Json(ClientError::from(ClientErrorCode::InvalidGrant)),
            ),
            Self::UnsupportedGrantType => (
                StatusCode::BAD_REQUEST,
                Json(ClientError::from(ClientErrorCode::UnsupportedGrantType)),
            ),
        }
        .into_response()
    }
}

impl_from_error_for_route!(mas_storage::RepositoryError);
impl_from_error_for_route!(super::IdTokenSignatureError);

#[tracing::instrument(
    name = "handlers.oauth2.token.post",
    fields(client.id = client_authorization.client_id()),
    skip_all,
    err,
)]
pub(crate) async fn post(
    mut rng: BoxRng,
    clock: BoxClock,
    State(http_client_factory): State<HttpClientFactory>,
    State(key_store): State<Keystore>,
    State(url_builder): State<UrlBuilder>,
    mut repo: BoxRepository,
    State(encrypter): State<Encrypter>,
    client_authorization: ClientAuthorization<AccessTokenRequest>,
) -> Result<impl IntoResponse, RouteError> {
    let client = client_authorization
        .credentials
        .fetch(&mut repo)
        .await?
        .ok_or(RouteError::ClientNotFound)?;

    let method = client
        .token_endpoint_auth_method
        .as_ref()
        .ok_or(RouteError::ClientNotAllowed)?;

    client_authorization
        .credentials
        .verify(&http_client_factory, &encrypter, method, &client)
        .await?;

    let form = client_authorization.form.ok_or(RouteError::BadRequest)?;

    let (reply, repo) = match form {
        AccessTokenRequest::AuthorizationCode(grant) => {
            authorization_code_grant(
                &mut rng,
                &clock,
                &grant,
                &client,
                &key_store,
                &url_builder,
                repo,
            )
            .await?
        }
        AccessTokenRequest::RefreshToken(grant) => {
            refresh_token_grant(&mut rng, &clock, &grant, &client, repo).await?
        }
        _ => {
            return Err(RouteError::UnsupportedGrantType);
        }
    };

    repo.save().await?;

    let mut headers = HeaderMap::new();
    headers.typed_insert(CacheControl::new().with_no_store());
    headers.typed_insert(Pragma::no_cache());

    Ok((headers, Json(reply)))
}

async fn authorization_code_grant(
    mut rng: &mut BoxRng,
    clock: &impl Clock,
    grant: &AuthorizationCodeGrant,
    client: &Client,
    key_store: &Keystore,
    url_builder: &UrlBuilder,
    mut repo: BoxRepository,
) -> Result<(AccessTokenResponse, BoxRepository), RouteError> {
    let authz_grant = repo
        .oauth2_authorization_grant()
        .find_by_code(&grant.code)
        .await?
        .ok_or(RouteError::GrantNotFound)?;

    let now = clock.now();

    let session_id = match authz_grant.stage {
        AuthorizationGrantStage::Cancelled { cancelled_at } => {
            debug!(%cancelled_at, "Authorization grant was cancelled");
            return Err(RouteError::InvalidGrant);
        }
        AuthorizationGrantStage::Exchanged {
            exchanged_at,
            fulfilled_at,
            session_id,
        } => {
            debug!(%exchanged_at, %fulfilled_at, "Authorization code was already exchanged");

            // Ending the session if the token was already exchanged more than 20s ago
            if now - exchanged_at > Duration::seconds(20) {
                debug!("Ending potentially compromised session");
                let session = repo
                    .oauth2_session()
                    .lookup(session_id)
                    .await?
                    .ok_or(RouteError::NoSuchOAuthSession)?;
                repo.oauth2_session().finish(clock, session).await?;
                repo.save().await?;
            }

            return Err(RouteError::InvalidGrant);
        }
        AuthorizationGrantStage::Pending => {
            debug!("Authorization grant has not been fulfilled yet");
            return Err(RouteError::InvalidGrant);
        }
        AuthorizationGrantStage::Fulfilled {
            session_id,
            fulfilled_at,
        } => {
            if now - fulfilled_at > Duration::minutes(10) {
                debug!("Code exchange took more than 10 minutes");
                return Err(RouteError::InvalidGrant);
            }

            session_id
        }
    };

    let session = repo
        .oauth2_session()
        .lookup(session_id)
        .await?
        .ok_or(RouteError::NoSuchOAuthSession)?;

    // This should never happen, since we looked up in the database using the code
    let code = authz_grant.code.as_ref().ok_or(RouteError::InvalidGrant)?;

    if client.id != session.client_id {
        return Err(RouteError::UnauthorizedClient);
    }

    match (code.pkce.as_ref(), grant.code_verifier.as_ref()) {
        (None, None) => {}
        // We have a challenge but no verifier (or vice-versa)? Bad request.
        (Some(_), None) | (None, Some(_)) => return Err(RouteError::BadRequest),
        // If we have both, we need to check the code validity
        (Some(pkce), Some(verifier)) => {
            pkce.verify(verifier)?;
        }
    };

    let browser_session = repo
        .browser_session()
        .lookup(session.user_session_id)
        .await?
        .ok_or(RouteError::NoSuchBrowserSession)?;

    let ttl = Duration::minutes(5);
    let (access_token, refresh_token) =
        generate_token_pair(&mut rng, clock, &mut repo, &session, ttl).await?;

    let id_token = if session.scope.contains(&scope::OPENID) {
        Some(generate_id_token(
            &mut rng,
            clock,
            url_builder,
            key_store,
            client,
            &authz_grant,
            &browser_session,
            Some(&access_token),
        )?)
    } else {
        None
    };

    let mut params = AccessTokenResponse::new(access_token.access_token)
        .with_expires_in(ttl)
        .with_refresh_token(refresh_token.refresh_token)
        .with_scope(session.scope.clone());

    if let Some(id_token) = id_token {
        params = params.with_id_token(id_token);
    }

    repo.oauth2_authorization_grant()
        .exchange(clock, authz_grant)
        .await?;

    Ok((params, repo))
}

async fn refresh_token_grant(
    rng: &mut BoxRng,
    clock: &impl Clock,
    grant: &RefreshTokenGrant,
    client: &Client,
    mut repo: BoxRepository,
) -> Result<(AccessTokenResponse, BoxRepository), RouteError> {
    let refresh_token = repo
        .oauth2_refresh_token()
        .find_by_token(&grant.refresh_token)
        .await?
        .ok_or(RouteError::InvalidGrant)?;

    let session = repo
        .oauth2_session()
        .lookup(refresh_token.session_id)
        .await?
        .ok_or(RouteError::NoSuchOAuthSession)?;

    if !refresh_token.is_valid() || !session.is_valid() {
        return Err(RouteError::InvalidGrant);
    }

    if client.id != session.client_id {
        // As per https://datatracker.ietf.org/doc/html/rfc6749#section-5.2
        return Err(RouteError::InvalidGrant);
    }

    let ttl = Duration::minutes(5);
    let (new_access_token, new_refresh_token) =
        generate_token_pair(rng, clock, &mut repo, &session, ttl).await?;

    let refresh_token = repo
        .oauth2_refresh_token()
        .consume(clock, refresh_token)
        .await?;

    if let Some(access_token_id) = refresh_token.access_token_id {
        let access_token = repo.oauth2_access_token().lookup(access_token_id).await?;
        if let Some(access_token) = access_token {
            repo.oauth2_access_token()
                .revoke(clock, access_token)
                .await?;
        }
    }

    let params = AccessTokenResponse::new(new_access_token.access_token)
        .with_expires_in(ttl)
        .with_refresh_token(new_refresh_token.refresh_token)
        .with_scope(session.scope);

    Ok((params, repo))
}

#[cfg(test)]
mod tests {
    use hyper::Request;
    use mas_data_model::{AccessToken, AuthorizationCode, RefreshToken};
    use mas_router::SimpleRoute;
    use oauth2_types::{
        registration::ClientRegistrationResponse,
        requests::ResponseMode,
        scope::{Scope, OPENID},
    };
    use sqlx::PgPool;

    use super::*;
    use crate::test_utils::{init_tracing, RequestBuilderExt, ResponseExt, TestState};

    #[sqlx::test(migrator = "mas_storage_pg::MIGRATOR")]
    async fn test_auth_code_grant(pool: PgPool) {
        init_tracing();
        let state = TestState::from_pool(pool).await.unwrap();

        // Provision a client
        let request =
            Request::post(mas_router::OAuth2RegistrationEndpoint::PATH).json(serde_json::json!({
                "client_uri": "https://example.com/",
                "redirect_uris": ["https://example.com/callback"],
                "contacts": ["contact@example.com"],
                "token_endpoint_auth_method": "none",
                "response_types": ["code"],
                "grant_types": ["authorization_code"],
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::CREATED);

        let ClientRegistrationResponse { client_id, .. } = response.json();

        // Let's provision a user and create a session for them. This part is hard to
        // test with just HTTP requests, so we'll use the repository directly.
        let mut repo = state.repository().await.unwrap();

        let user = repo
            .user()
            .add(&mut state.rng(), &state.clock, "alice".to_owned())
            .await
            .unwrap();

        let browser_session = repo
            .browser_session()
            .add(&mut state.rng(), &state.clock, &user)
            .await
            .unwrap();

        // Lookup the client in the database.
        let client = repo
            .oauth2_client()
            .find_by_client_id(&client_id)
            .await
            .unwrap()
            .unwrap();

        // Start a grant
        let code = "thisisaverysecurecode";
        let grant = repo
            .oauth2_authorization_grant()
            .add(
                &mut state.rng(),
                &state.clock,
                &client,
                "https://example.com/redirect".parse().unwrap(),
                Scope::from_iter([OPENID]),
                Some(AuthorizationCode {
                    code: code.to_owned(),
                    pkce: None,
                }),
                Some("state".to_owned()),
                Some("nonce".to_owned()),
                None,
                ResponseMode::Query,
                false,
                false,
            )
            .await
            .unwrap();

        let session = repo
            .oauth2_session()
            .add(
                &mut state.rng(),
                &state.clock,
                &client,
                &browser_session,
                grant.scope.clone(),
            )
            .await
            .unwrap();

        // And fulfill it
        let grant = repo
            .oauth2_authorization_grant()
            .fulfill(&state.clock, &session, grant)
            .await
            .unwrap();

        repo.save().await.unwrap();

        // Now call the token endpoint to get an access token.
        let request =
            Request::post(mas_router::OAuth2TokenEndpoint::PATH).form(serde_json::json!({
                "grant_type": "authorization_code",
                "code": code,
                "redirect_uri": grant.redirect_uri,
                "client_id": client.client_id,
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::OK);

        let AccessTokenResponse { access_token, .. } = response.json();

        // Check that the token is valid
        assert!(state.is_access_token_valid(&access_token).await);

        // Exchange it again, this it should fail
        let request =
            Request::post(mas_router::OAuth2TokenEndpoint::PATH).form(serde_json::json!({
                "grant_type": "authorization_code",
                "code": code,
                "redirect_uri": grant.redirect_uri,
                "client_id": client.client_id,
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::BAD_REQUEST);
        let error: ClientError = response.json();
        assert_eq!(error.error, ClientErrorCode::InvalidGrant);

        // The token should still be valid
        assert!(state.is_access_token_valid(&access_token).await);

        // Now wait a bit
        state.clock.advance(Duration::minutes(1));

        // Exchange it again, this it should fail
        let request =
            Request::post(mas_router::OAuth2TokenEndpoint::PATH).form(serde_json::json!({
                "grant_type": "authorization_code",
                "code": code,
                "redirect_uri": grant.redirect_uri,
                "client_id": client.client_id,
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::BAD_REQUEST);
        let error: ClientError = response.json();
        assert_eq!(error.error, ClientErrorCode::InvalidGrant);

        // And it should have revoked the token we got
        assert!(!state.is_access_token_valid(&access_token).await);

        // Try another one and wait for too long before exchanging it
        let mut repo = state.repository().await.unwrap();
        let code = "thisisanothercode";
        let grant = repo
            .oauth2_authorization_grant()
            .add(
                &mut state.rng(),
                &state.clock,
                &client,
                "https://example.com/redirect".parse().unwrap(),
                Scope::from_iter([OPENID]),
                Some(AuthorizationCode {
                    code: code.to_owned(),
                    pkce: None,
                }),
                Some("state".to_owned()),
                Some("nonce".to_owned()),
                None,
                ResponseMode::Query,
                false,
                false,
            )
            .await
            .unwrap();

        let session = repo
            .oauth2_session()
            .add(
                &mut state.rng(),
                &state.clock,
                &client,
                &browser_session,
                grant.scope.clone(),
            )
            .await
            .unwrap();

        // And fulfill it
        let grant = repo
            .oauth2_authorization_grant()
            .fulfill(&state.clock, &session, grant)
            .await
            .unwrap();

        repo.save().await.unwrap();

        // Now wait a bit
        state.clock.advance(Duration::minutes(15));

        // Exchange it, it should fail
        let request =
            Request::post(mas_router::OAuth2TokenEndpoint::PATH).form(serde_json::json!({
                "grant_type": "authorization_code",
                "code": code,
                "redirect_uri": grant.redirect_uri,
                "client_id": client.client_id,
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::BAD_REQUEST);
        let ClientError { error, .. } = response.json();
        assert_eq!(error, ClientErrorCode::InvalidGrant);
    }

    #[sqlx::test(migrator = "mas_storage_pg::MIGRATOR")]
    async fn test_refresh_token_grant(pool: PgPool) {
        init_tracing();
        let state = TestState::from_pool(pool).await.unwrap();

        // Provision a client
        let request =
            Request::post(mas_router::OAuth2RegistrationEndpoint::PATH).json(serde_json::json!({
                "client_uri": "https://example.com/",
                "redirect_uris": ["https://example.com/callback"],
                "contacts": ["contact@example.com"],
                "token_endpoint_auth_method": "none",
                "response_types": ["code"],
                "grant_types": ["authorization_code"],
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::CREATED);

        let ClientRegistrationResponse { client_id, .. } = response.json();

        // Let's provision a user and create a session for them. This part is hard to
        // test with just HTTP requests, so we'll use the repository directly.
        let mut repo = state.repository().await.unwrap();

        let user = repo
            .user()
            .add(&mut state.rng(), &state.clock, "alice".to_owned())
            .await
            .unwrap();

        let browser_session = repo
            .browser_session()
            .add(&mut state.rng(), &state.clock, &user)
            .await
            .unwrap();

        // Lookup the client in the database.
        let client = repo
            .oauth2_client()
            .find_by_client_id(&client_id)
            .await
            .unwrap()
            .unwrap();

        // Get a token pair
        let session = repo
            .oauth2_session()
            .add(
                &mut state.rng(),
                &state.clock,
                &client,
                &browser_session,
                Scope::from_iter([OPENID]),
            )
            .await
            .unwrap();

        let (AccessToken { access_token, .. }, RefreshToken { refresh_token, .. }) =
            generate_token_pair(
                &mut state.rng(),
                &state.clock,
                &mut repo,
                &session,
                Duration::minutes(5),
            )
            .await
            .unwrap();

        repo.save().await.unwrap();

        // First check that the token is valid
        assert!(state.is_access_token_valid(&access_token).await);

        // Now call the token endpoint to get an access token.
        let request =
            Request::post(mas_router::OAuth2TokenEndpoint::PATH).form(serde_json::json!({
                "grant_type": "refresh_token",
                "refresh_token": refresh_token,
                "client_id": client.client_id,
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::OK);

        let old_access_token = access_token;
        let old_refresh_token = refresh_token;
        let response: AccessTokenResponse = response.json();
        let access_token = response.access_token;
        let refresh_token = response.refresh_token.expect("to have a refresh token");

        // Check that the new token is valid
        assert!(state.is_access_token_valid(&access_token).await);

        // Check that the old token is no longer valid
        assert!(!state.is_access_token_valid(&old_access_token).await);

        // Call it again with the old token, it should fail
        let request =
            Request::post(mas_router::OAuth2TokenEndpoint::PATH).form(serde_json::json!({
                "grant_type": "refresh_token",
                "refresh_token": old_refresh_token,
                "client_id": client.client_id,
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::BAD_REQUEST);
        let ClientError { error, .. } = response.json();
        assert_eq!(error, ClientErrorCode::InvalidGrant);

        // Call it again with the new token, it should work
        let request =
            Request::post(mas_router::OAuth2TokenEndpoint::PATH).form(serde_json::json!({
                "grant_type": "refresh_token",
                "refresh_token": refresh_token,
                "client_id": client.client_id,
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::OK);
        let _: AccessTokenResponse = response.json();
    }

    #[sqlx::test(migrator = "mas_storage_pg::MIGRATOR")]
    async fn test_unsupported_grant(pool: PgPool) {
        init_tracing();
        let state = TestState::from_pool(pool).await.unwrap();

        // Provision a client
        let request =
            Request::post(mas_router::OAuth2RegistrationEndpoint::PATH).json(serde_json::json!({
                "client_uri": "https://example.com/",
                "redirect_uris": ["https://example.com/callback"],
                "contacts": ["contact@example.com"],
                "token_endpoint_auth_method": "client_secret_post",
                "grant_types": ["client_credentials"],
                "response_types": [],
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::CREATED);

        let response: ClientRegistrationResponse = response.json();
        let client_id = response.client_id;
        let client_secret = response.client_secret.expect("to have a client secret");

        // Call the token endpoint with an unsupported grant type
        let request =
            Request::post(mas_router::OAuth2TokenEndpoint::PATH).form(serde_json::json!({
                "grant_type": "client_credentials",
                "client_id": client_id,
                "client_secret": client_secret,
            }));

        let response = state.request(request).await;
        response.assert_status(StatusCode::BAD_REQUEST);
        let ClientError { error, .. } = response.json();
        assert_eq!(error, ClientErrorCode::UnsupportedGrantType);
    }
}
