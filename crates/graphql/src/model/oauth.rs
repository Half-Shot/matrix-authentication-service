// Copyright 2022 The Matrix.org Foundation C.I.C.
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

use anyhow::Context as _;
use async_graphql::{Context, Description, Object, ID};
use mas_storage::{oauth2::OAuth2ClientRepository, user::BrowserSessionRepository, BoxRepository};
use oauth2_types::scope::Scope;
use tokio::sync::Mutex;
use ulid::Ulid;
use url::Url;

use super::{BrowserSession, NodeType, User};

/// An OAuth 2.0 session represents a client session which used the OAuth APIs
/// to login.
#[derive(Description)]
pub struct OAuth2Session(pub mas_data_model::Session);

#[Object(use_type_description)]
impl OAuth2Session {
    /// ID of the object.
    pub async fn id(&self) -> ID {
        NodeType::OAuth2Session.id(self.0.id)
    }

    /// OAuth 2.0 client used by this session.
    pub async fn client(&self, ctx: &Context<'_>) -> Result<OAuth2Client, async_graphql::Error> {
        let mut repo = ctx.data::<Mutex<BoxRepository>>()?.lock().await;
        let client = repo
            .oauth2_client()
            .lookup(self.0.client_id)
            .await?
            .context("Could not load client")?;

        Ok(OAuth2Client(client))
    }

    /// Scope granted for this session.
    pub async fn scope(&self) -> String {
        self.0.scope.to_string()
    }

    /// The browser session which started this OAuth 2.0 session.
    pub async fn browser_session(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BrowserSession, async_graphql::Error> {
        let mut repo = ctx.data::<Mutex<BoxRepository>>()?.lock().await;
        let browser_session = repo
            .browser_session()
            .lookup(self.0.user_session_id)
            .await?
            .context("Could not load browser session")?;

        Ok(BrowserSession(browser_session))
    }

    /// User authorized for this session.
    pub async fn user(&self, ctx: &Context<'_>) -> Result<User, async_graphql::Error> {
        let mut repo = ctx.data::<Mutex<BoxRepository>>()?.lock().await;
        let browser_session = repo
            .browser_session()
            .lookup(self.0.user_session_id)
            .await?
            .context("Could not load browser session")?;

        Ok(User(browser_session.user))
    }
}

/// An OAuth 2.0 client
#[derive(Description)]
pub struct OAuth2Client(pub mas_data_model::Client);

#[Object(use_type_description)]
impl OAuth2Client {
    /// ID of the object.
    pub async fn id(&self) -> ID {
        NodeType::OAuth2Client.id(self.0.id)
    }

    /// OAuth 2.0 client ID
    pub async fn client_id(&self) -> &str {
        &self.0.client_id
    }

    /// Client name advertised by the client.
    pub async fn client_name(&self) -> Option<&str> {
        self.0.client_name.as_deref()
    }

    /// Client URI advertised by the client.
    pub async fn client_uri(&self) -> Option<&Url> {
        self.0.client_uri.as_ref()
    }

    /// Terms of services URI advertised by the client.
    pub async fn tos_uri(&self) -> Option<&Url> {
        self.0.tos_uri.as_ref()
    }

    /// Privacy policy URI advertised by the client.
    pub async fn policy_uri(&self) -> Option<&Url> {
        self.0.policy_uri.as_ref()
    }

    /// List of redirect URIs used for authorization grants by the client.
    pub async fn redirect_uris(&self) -> &[Url] {
        &self.0.redirect_uris
    }
}

/// An OAuth 2.0 consent represents the scope a user consented to grant to a
/// client.
#[derive(Description)]
pub struct OAuth2Consent {
    scope: Scope,
    client_id: Ulid,
}

#[Object(use_type_description)]
impl OAuth2Consent {
    /// Scope consented by the user for this client.
    pub async fn scope(&self) -> String {
        self.scope.to_string()
    }

    /// OAuth 2.0 client for which the user granted access.
    pub async fn client(&self, ctx: &Context<'_>) -> Result<OAuth2Client, async_graphql::Error> {
        let mut repo = ctx.data::<Mutex<BoxRepository>>()?.lock().await;
        let client = repo
            .oauth2_client()
            .lookup(self.client_id)
            .await?
            .context("Could not load client")?;
        Ok(OAuth2Client(client))
    }
}
