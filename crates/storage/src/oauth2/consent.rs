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

use std::{collections::BTreeSet, str::FromStr};

use chrono::{DateTime, Utc};
use mas_data_model::{Client, ClientConsent, User};
use oauth2_types::scope::{Scope, ScopeToken};
use rand::Rng;
use sqlx::{PgConnection, PgExecutor, QueryBuilder};
use tracing::{info_span, Instrument};
use ulid::Ulid;
use uuid::Uuid;

use crate::{
    pagination::{process_page, QueryBuilderExt},
    user::lookup_user,
    Clock, DatabaseError, DatabaseInconsistencyError,
};

use super::client::{lookup_client, lookup_clients};

#[tracing::instrument(
    skip_all,
    fields(
        %user.id,
        %client.id,
    ),
    err,
)]
pub async fn fetch_client_consent(
    executor: impl PgExecutor<'_>,
    user: &User,
    client: &Client,
) -> Result<Scope, DatabaseError> {
    let scope_tokens: Vec<String> = sqlx::query_scalar!(
        r#"
            SELECT scope_token
            FROM oauth2_consents
            WHERE user_id = $1 AND oauth2_client_id = $2
        "#,
        Uuid::from(user.id),
        Uuid::from(client.id),
    )
    .fetch_all(executor)
    .await?;

    let scope: Result<Scope, _> = scope_tokens
        .into_iter()
        .map(|s| ScopeToken::from_str(&s))
        .collect();

    let scope = scope.map_err(|e| {
        DatabaseInconsistencyError::on("oauth2_consents")
            .column("scope_token")
            .source(e)
    })?;

    Ok(scope)
}

#[tracing::instrument(
    skip_all,
    fields(
        %user.id,
        %client.id,
        %scope,
    ),
    err,
)]
pub async fn insert_client_consent(
    executor: impl PgExecutor<'_>,
    mut rng: impl Rng + Send,
    clock: &Clock,
    user: &User,
    client: &Client,
    scope: &Scope,
) -> Result<(), sqlx::Error> {
    let now = clock.now();
    let (tokens, ids): (Vec<String>, Vec<Uuid>) = scope
        .iter()
        .map(|token| {
            (
                token.to_string(),
                Uuid::from(Ulid::from_datetime_with_source(now.into(), &mut rng)),
            )
        })
        .unzip();

    sqlx::query!(
        r#"
            INSERT INTO oauth2_consents
                (oauth2_consent_id, user_id, oauth2_client_id, scope_token, created_at)
            SELECT id, $2, $3, scope_token, $5 FROM UNNEST($1::uuid[], $4::text[]) u(id, scope_token)
            ON CONFLICT (user_id, oauth2_client_id, scope_token) DO UPDATE SET refreshed_at = $5
        "#,
        &ids,
        Uuid::from(user.id),
        Uuid::from(client.id),
        &tokens,
        now,
    )
    .execute(executor)
    .await?;

    Ok(())
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct OAuth2ConsentLookup {
    oauth2_consent_id: Uuid,
    oauth2_client_id: Uuid,
    user_id: Uuid,
    scope_token: String,
    created_at: DateTime<Utc>,
    refreshed_at: Option<DateTime<Utc>>,
}

#[tracing::instrument(
    skip_all,
    fields(consent.id = %id),
    err,
)]
pub async fn lookup_consent(
    conn: &mut PgConnection,
    id: Ulid,
) -> Result<Option<ClientConsent>, DatabaseError> {
    let res = sqlx::query_as!(
        OAuth2ConsentLookup,
        r#"
            SELECT
                c.oauth2_consent_id,
                c.oauth2_client_id,
                c.user_id,
                c.scope_token,
                c.created_at,
                c.refreshed_at
            FROM oauth2_consents c
            WHERE c.oauth2_consent_id = $1
        "#,
        Uuid::from(id),
    )
    .fetch_one(&mut *conn)
    .await;

    let Ok(res) = res else { return Ok(None) };

    let id = Ulid::from(res.oauth2_consent_id);
    let client = lookup_client(&mut *conn, Ulid::from(res.oauth2_client_id))
        .await?
        .ok_or_else(|| {
            DatabaseInconsistencyError::on("oauth2_consents")
                .column("client_id")
                .row(id)
        })?;
    let scope = res.scope_token.parse().map_err(|e| {
        DatabaseInconsistencyError::on("oauth2_consents")
            .column("scope_token")
            .row(id)
            .source(e)
    })?;

    let user = lookup_user(&mut *conn, Ulid::from(res.user_id)).await?;

    Ok(Some(ClientConsent {
        id: Ulid::from(res.oauth2_consent_id),
        user: user,
        client,
        scope,
        created_at: res.created_at,
        refreshed_at: res.refreshed_at,
    }))
}

#[tracing::instrument(skip_all, err)]
pub async fn get_paginated_user_client_consents(
    conn: &mut PgConnection,
    user: &User,
    before: Option<Ulid>,
    after: Option<Ulid>,
    first: Option<usize>,
    last: Option<usize>,
) -> Result<(bool, bool, Vec<ClientConsent>), DatabaseError> {
    let mut query = QueryBuilder::new(
        r#"
            SELECT
                c.oauth2_consent_id,
                c.oauth2_client_id,
                c.user_id,
                c.scope_token,
                c.created_at,
                c.refreshed_at
            FROM oauth2_consents c
        "#,
    );

    query
        .push(" WHERE c.user_id = ")
        .push_bind(Uuid::from(user.id))
        .generate_pagination("oauth2_consent_id", before, after, first, last)?;

    let span = info_span!(
        "Fetch paginated user client consents",
        db.statement = query.sql()
    );
    let page: Vec<OAuth2ConsentLookup> = query
        .build_query_as()
        .fetch_all(&mut *conn)
        .instrument(span)
        .await?;

    let (has_previous_page, has_next_page, page) = process_page(page, first, last)?;

    let client_ids: BTreeSet<Ulid> = page
        .iter()
        .map(|i| Ulid::from(i.oauth2_client_id))
        .collect();

    let clients = lookup_clients(&mut *conn, client_ids).await?;

    let page: Result<Vec<_>, DatabaseInconsistencyError> = page
        .into_iter()
        .map(|item| {
            let id = Ulid::from(item.oauth2_consent_id);
            let client = clients
                .get(&Ulid::from(item.oauth2_client_id))
                .ok_or_else(|| {
                    DatabaseInconsistencyError::on("oauth2_consents")
                        .column("oauth2_client_id")
                        .row(id)
                })?
                .clone();

            let scope = item.scope_token.parse().map_err(|e| {
                DatabaseInconsistencyError::on("oauth2_consents")
                    .column("scope_token")
                    .row(id)
                    .source(e)
            })?;

            Ok(ClientConsent {
                id,
                client,
                scope,
                user: user.clone(),
                created_at: item.created_at,
                refreshed_at: item.refreshed_at,
            })
        })
        .collect();

    Ok((has_previous_page, has_next_page, page?))
}
