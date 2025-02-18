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

#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::str_to_string, rustdoc::broken_intra_doc_links)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]

use thiserror::Error;

pub(crate) mod compat;
pub(crate) mod oauth2;
pub(crate) mod tokens;
pub(crate) mod upstream_oauth2;
pub(crate) mod users;

#[derive(Debug, Error)]
#[error("invalid state transition")]
pub struct InvalidTransitionError;

pub use self::{
    compat::{
        CompatAccessToken, CompatRefreshToken, CompatRefreshTokenState, CompatSession,
        CompatSessionState, CompatSsoLogin, CompatSsoLoginState, Device,
    },
    oauth2::{
        AuthorizationCode, AuthorizationGrant, AuthorizationGrantStage, Client,
        InvalidRedirectUriError, JwksOrJwksUri, Pkce, Session, SessionState,
    },
    tokens::{
        AccessToken, AccessTokenState, RefreshToken, RefreshTokenState, TokenFormatError, TokenType,
    },
    upstream_oauth2::{
        UpstreamOAuthAuthorizationSession, UpstreamOAuthAuthorizationSessionState,
        UpstreamOAuthLink, UpstreamOAuthProvider,
    },
    users::{
        Authentication, BrowserSession, Password, User, UserEmail, UserEmailVerification,
        UserEmailVerificationState,
    },
};
