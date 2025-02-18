// Copyright 2021, 2022 The Matrix.org Foundation C.I.C.
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

#![forbid(unsafe_code)]
#![deny(
    clippy::all,
    clippy::str_to_string,
    rustdoc::broken_intra_doc_links,
    clippy::future_not_send
)]
#![warn(clippy::pedantic)]
#![allow(
    // Some axum handlers need that
    clippy::unused_async,
    // Because of how axum handlers work, we sometime have take many arguments
    clippy::too_many_arguments,
)]

use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::{
    body::{Bytes, HttpBody},
    extract::{FromRef, FromRequestParts},
    response::{Html, IntoResponse},
    routing::{get, on, post, MethodFilter},
    Router,
};
use headers::HeaderName;
use hyper::header::{ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_LANGUAGE, CONTENT_TYPE};
use mas_http::CorsLayerExt;
use mas_keystore::{Encrypter, Keystore};
use mas_policy::PolicyFactory;
use mas_router::{Route, UrlBuilder};
use mas_storage::{BoxClock, BoxRepository, BoxRng};
use mas_templates::{ErrorContext, Templates};
use passwords::PasswordManager;
use sqlx::PgPool;
use tower::util::AndThenLayer;
use tower_http::cors::{Any, CorsLayer};

mod app_state;
mod compat;
mod graphql;
mod health;
mod oauth2;
pub mod passwords;
mod upstream_oauth2;
mod views;

#[cfg(test)]
mod test_utils;

/// Implement `From<E>` for `RouteError`, for "internal server error" kind of
/// errors.
#[macro_export]
macro_rules! impl_from_error_for_route {
    ($route_error:ty : $error:ty) => {
        impl From<$error> for $route_error {
            fn from(e: $error) -> Self {
                Self::Internal(Box::new(e))
            }
        }
    };
    ($error:ty) => {
        impl_from_error_for_route!(self::RouteError: $error);
    };
}

pub use mas_axum_utils::http_client_factory::HttpClientFactory;

pub use self::{app_state::AppState, compat::MatrixHomeserver, graphql::schema as graphql_schema};

pub fn healthcheck_router<S, B>() -> Router<S, B>
where
    B: HttpBody + Send + 'static,
    S: Clone + Send + Sync + 'static,
    PgPool: FromRef<S>,
{
    Router::new().route(mas_router::Healthcheck::route(), get(self::health::get))
}

pub fn graphql_router<S, B>(playground: bool) -> Router<S, B>
where
    B: HttpBody + Send + 'static,
    <B as HttpBody>::Data: Into<Bytes>,
    <B as HttpBody>::Error: std::error::Error + Send + Sync,
    S: Clone + Send + Sync + 'static,
    mas_graphql::Schema: FromRef<S>,
    BoxRepository: FromRequestParts<S>,
    Encrypter: FromRef<S>,
{
    let mut router = Router::new().route(
        "/graphql",
        get(self::graphql::get).post(self::graphql::post),
    );

    if playground {
        router = router.route("/graphql/playground", get(self::graphql::playground));
    }

    router
}

pub fn discovery_router<S, B>() -> Router<S, B>
where
    B: HttpBody + Send + 'static,
    S: Clone + Send + Sync + 'static,
    Keystore: FromRef<S>,
    UrlBuilder: FromRef<S>,
    BoxClock: FromRequestParts<S>,
    BoxRng: FromRequestParts<S>,
{
    Router::new()
        .route(
            mas_router::OidcConfiguration::route(),
            get(self::oauth2::discovery::get),
        )
        .route(
            mas_router::Webfinger::route(),
            get(self::oauth2::webfinger::get),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_otel_headers([
                    AUTHORIZATION,
                    ACCEPT,
                    ACCEPT_LANGUAGE,
                    CONTENT_LANGUAGE,
                    CONTENT_TYPE,
                ])
                .max_age(Duration::from_secs(60 * 60)),
        )
}

pub fn api_router<S, B>() -> Router<S, B>
where
    B: HttpBody + Send + 'static,
    <B as HttpBody>::Data: Send,
    <B as HttpBody>::Error: std::error::Error + Send + Sync,
    S: Clone + Send + Sync + 'static,
    Keystore: FromRef<S>,
    UrlBuilder: FromRef<S>,
    Arc<PolicyFactory>: FromRef<S>,
    BoxRepository: FromRequestParts<S>,
    Encrypter: FromRef<S>,
    HttpClientFactory: FromRef<S>,
    BoxClock: FromRequestParts<S>,
    BoxRng: FromRequestParts<S>,
{
    // All those routes are API-like, with a common CORS layer
    Router::new()
        .route(
            mas_router::OAuth2Keys::route(),
            get(self::oauth2::keys::get),
        )
        .route(
            mas_router::OidcUserinfo::route(),
            on(
                MethodFilter::POST | MethodFilter::GET,
                self::oauth2::userinfo::get,
            ),
        )
        .route(
            mas_router::OAuth2Introspection::route(),
            post(self::oauth2::introspection::post),
        )
        .route(
            mas_router::OAuth2Revocation::route(),
            post(self::oauth2::revoke::post),
        )
        .route(
            mas_router::OAuth2TokenEndpoint::route(),
            post(self::oauth2::token::post),
        )
        .route(
            mas_router::OAuth2RegistrationEndpoint::route(),
            post(self::oauth2::registration::post),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_otel_headers([
                    AUTHORIZATION,
                    ACCEPT,
                    ACCEPT_LANGUAGE,
                    CONTENT_LANGUAGE,
                    CONTENT_TYPE,
                ])
                .max_age(Duration::from_secs(60 * 60)),
        )
}

#[allow(clippy::trait_duplication_in_bounds)]
pub fn compat_router<S, B>() -> Router<S, B>
where
    B: HttpBody + Send + 'static,
    <B as HttpBody>::Data: Send,
    <B as HttpBody>::Error: std::error::Error + Send + Sync,
    S: Clone + Send + Sync + 'static,
    UrlBuilder: FromRef<S>,
    BoxRepository: FromRequestParts<S>,
    MatrixHomeserver: FromRef<S>,
    PasswordManager: FromRef<S>,
    BoxClock: FromRequestParts<S>,
    BoxRng: FromRequestParts<S>,
{
    Router::new()
        .route(
            mas_router::CompatLogin::route(),
            get(self::compat::login::get).post(self::compat::login::post),
        )
        .route(
            mas_router::CompatLogout::route(),
            post(self::compat::logout::post),
        )
        .route(
            mas_router::CompatRefresh::route(),
            post(self::compat::refresh::post),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_otel_headers([
                    AUTHORIZATION,
                    ACCEPT,
                    ACCEPT_LANGUAGE,
                    CONTENT_LANGUAGE,
                    CONTENT_TYPE,
                    HeaderName::from_static("x-requested-with"),
                ])
                .max_age(Duration::from_secs(60 * 60)),
        )
}

pub fn human_router<S, B>(templates: Templates) -> Router<S, B>
where
    B: HttpBody + Send + 'static,
    <B as HttpBody>::Data: Send,
    <B as HttpBody>::Error: std::error::Error + Send + Sync,
    S: Clone + Send + Sync + 'static,
    UrlBuilder: FromRef<S>,
    Arc<PolicyFactory>: FromRef<S>,
    BoxRepository: FromRequestParts<S>,
    Encrypter: FromRef<S>,
    Templates: FromRef<S>,
    Keystore: FromRef<S>,
    HttpClientFactory: FromRef<S>,
    PasswordManager: FromRef<S>,
    BoxClock: FromRequestParts<S>,
    BoxRng: FromRequestParts<S>,
{
    Router::new()
        .route(
            mas_router::ChangePasswordDiscovery::route(),
            get(|| async { mas_router::AccountPassword.go() }),
        )
        .route(mas_router::Index::route(), get(self::views::index::get))
        .route(
            mas_router::Login::route(),
            get(self::views::login::get).post(self::views::login::post),
        )
        .route(mas_router::Logout::route(), post(self::views::logout::post))
        .route(
            mas_router::Reauth::route(),
            get(self::views::reauth::get).post(self::views::reauth::post),
        )
        .route(
            mas_router::Register::route(),
            get(self::views::register::get).post(self::views::register::post),
        )
        .route(mas_router::Account::route(), get(self::views::account::get))
        .route(
            mas_router::AccountPassword::route(),
            get(self::views::account::password::get).post(self::views::account::password::post),
        )
        .route(
            mas_router::AccountEmails::route(),
            get(self::views::account::emails::get).post(self::views::account::emails::post),
        )
        .route(
            mas_router::AccountVerifyEmail::route(),
            get(self::views::account::emails::verify::get)
                .post(self::views::account::emails::verify::post),
        )
        .route(
            mas_router::AccountAddEmail::route(),
            get(self::views::account::emails::add::get)
                .post(self::views::account::emails::add::post),
        )
        .route(
            mas_router::OAuth2AuthorizationEndpoint::route(),
            get(self::oauth2::authorization::get),
        )
        .route(
            mas_router::ContinueAuthorizationGrant::route(),
            get(self::oauth2::authorization::complete::get),
        )
        .route(
            mas_router::Consent::route(),
            get(self::oauth2::consent::get).post(self::oauth2::consent::post),
        )
        .route(
            mas_router::CompatLoginSsoRedirect::route(),
            get(self::compat::login_sso_redirect::get),
        )
        .route(
            mas_router::CompatLoginSsoRedirectIdp::route(),
            get(self::compat::login_sso_redirect::get),
        )
        .route(
            mas_router::CompatLoginSsoComplete::route(),
            get(self::compat::login_sso_complete::get).post(self::compat::login_sso_complete::post),
        )
        .route(
            mas_router::UpstreamOAuth2Authorize::route(),
            get(self::upstream_oauth2::authorize::get),
        )
        .route(
            mas_router::UpstreamOAuth2Callback::route(),
            get(self::upstream_oauth2::callback::get),
        )
        .route(
            mas_router::UpstreamOAuth2Link::route(),
            get(self::upstream_oauth2::link::get).post(self::upstream_oauth2::link::post),
        )
        .layer(AndThenLayer::new(
            move |response: axum::response::Response| async move {
                if response.status().is_server_error() {
                    // Error responses should have an ErrorContext attached to them
                    let ext = response.extensions().get::<ErrorContext>();
                    if let Some(ctx) = ext {
                        if let Ok(res) = templates.render_error(ctx).await {
                            let (mut parts, _original_body) = response.into_parts();
                            parts.headers.remove(CONTENT_TYPE);
                            return Ok((parts, Html(res)).into_response());
                        }
                    }
                }

                Ok::<_, Infallible>(response)
            },
        ))
}
