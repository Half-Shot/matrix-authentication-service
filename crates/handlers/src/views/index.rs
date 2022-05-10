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

use axum::{
    extract::Extension,
    response::{Html, IntoResponse},
};
use axum_extra::extract::PrivateCookieJar;
use mas_axum_utils::{csrf::CsrfExt, fancy_error, FancyError, SessionInfoExt};
use mas_config::Encrypter;
use mas_router::UrlBuilder;
use mas_templates::{IndexContext, TemplateContext, Templates};
use sqlx::PgPool;

pub async fn get(
    Extension(templates): Extension<Templates>,
    Extension(url_builder): Extension<UrlBuilder>,
    Extension(pool): Extension<PgPool>,
    cookie_jar: PrivateCookieJar<Encrypter>,
) -> Result<impl IntoResponse, FancyError> {
    let mut conn = pool
        .acquire()
        .await
        .map_err(fancy_error(templates.clone()))?;

    let (csrf_token, cookie_jar) = cookie_jar.csrf_token();
    let (session_info, cookie_jar) = cookie_jar.session_info();
    let session = session_info
        .load_session(&mut conn)
        .await
        .map_err(fancy_error(templates.clone()))?;

    let ctx = IndexContext::new(url_builder.oidc_discovery())
        .maybe_with_session(session)
        .with_csrf(csrf_token.form_value());

    let content = templates
        .render_index(&ctx)
        .await
        .map_err(fancy_error(templates))?;

    Ok((cookie_jar, Html(content)))
}
