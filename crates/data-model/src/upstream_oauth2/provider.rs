// Copyright 2023 The Matrix.org Foundation C.I.C.
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

use chrono::{DateTime, Utc};
use mas_iana::{jose::JsonWebSignatureAlg, oauth::OAuthClientAuthenticationMethod};
use oauth2_types::scope::Scope;
use serde::Serialize;
use ulid::Ulid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpstreamOAuthProvider {
    pub id: Ulid,
    pub issuer: String,
    pub scope: Scope,
    pub client_id: String,
    pub encrypted_client_secret: Option<String>,
    pub token_endpoint_signing_alg: Option<JsonWebSignatureAlg>,
    pub token_endpoint_auth_method: OAuthClientAuthenticationMethod,
    pub created_at: DateTime<Utc>,
}
