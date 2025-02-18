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

use anyhow::Context;
use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::PrivateCookieJar;
use mas_axum_utils::{
    csrf::{CsrfExt, ProtectedForm},
    FancyError, SessionInfoExt,
};
use mas_data_model::BrowserSession;
use mas_keystore::Encrypter;
use mas_router::Route;
use mas_storage::{
    user::{BrowserSessionRepository, UserPasswordRepository},
    BoxClock, BoxRepository, BoxRng, Clock,
};
use mas_templates::{EmptyContext, TemplateContext, Templates};
use rand::Rng;
use serde::Deserialize;
use zeroize::Zeroizing;

use crate::passwords::PasswordManager;

#[derive(Deserialize)]
pub struct ChangeForm {
    current_password: String,
    new_password: String,
    new_password_confirm: String,
}

#[tracing::instrument(name = "handlers.views.account_password.get", skip_all, err)]
pub(crate) async fn get(
    mut rng: BoxRng,
    clock: BoxClock,
    State(templates): State<Templates>,
    mut repo: BoxRepository,
    cookie_jar: PrivateCookieJar<Encrypter>,
) -> Result<Response, FancyError> {
    let (session_info, cookie_jar) = cookie_jar.session_info();

    let maybe_session = session_info.load_session(&mut repo).await?;

    if let Some(session) = maybe_session {
        render(&mut rng, &clock, templates, session, cookie_jar).await
    } else {
        let login = mas_router::Login::and_then(mas_router::PostAuthAction::ChangePassword);
        Ok((cookie_jar, login.go()).into_response())
    }
}

async fn render(
    rng: impl Rng + Send,
    clock: &impl Clock,
    templates: Templates,
    session: BrowserSession,
    cookie_jar: PrivateCookieJar<Encrypter>,
) -> Result<Response, FancyError> {
    let (csrf_token, cookie_jar) = cookie_jar.csrf_token(clock, rng);

    let ctx = EmptyContext
        .with_session(session)
        .with_csrf(csrf_token.form_value());

    let content = templates.render_account_password(&ctx).await?;

    Ok((cookie_jar, Html(content)).into_response())
}

#[tracing::instrument(name = "handlers.views.account_password.post", skip_all, err)]
pub(crate) async fn post(
    mut rng: BoxRng,
    clock: BoxClock,
    State(password_manager): State<PasswordManager>,
    State(templates): State<Templates>,
    mut repo: BoxRepository,
    cookie_jar: PrivateCookieJar<Encrypter>,
    Form(form): Form<ProtectedForm<ChangeForm>>,
) -> Result<Response, FancyError> {
    let form = cookie_jar.verify_form(&clock, form)?;

    let (session_info, cookie_jar) = cookie_jar.session_info();

    let maybe_session = session_info.load_session(&mut repo).await?;

    let Some(session) = maybe_session else {
        let login = mas_router::Login::and_then(mas_router::PostAuthAction::ChangePassword);
        return Ok((cookie_jar, login.go()).into_response());
    };

    let user_password = repo
        .user_password()
        .active(&session.user)
        .await?
        .context("user has no password")?;

    let password = Zeroizing::new(form.current_password.into_bytes());
    let new_password = Zeroizing::new(form.new_password.into_bytes());
    let new_password_confirm = Zeroizing::new(form.new_password_confirm.into_bytes());

    password_manager
        .verify(
            user_password.version,
            password,
            user_password.hashed_password,
        )
        .await?;

    // TODO: display nice form errors
    if new_password != new_password_confirm {
        return Err(anyhow::anyhow!("password mismatch").into());
    }

    let (version, hashed_password) = password_manager.hash(&mut rng, new_password).await?;
    let user_password = repo
        .user_password()
        .add(
            &mut rng,
            &clock,
            &session.user,
            version,
            hashed_password,
            None,
        )
        .await?;

    let session = repo
        .browser_session()
        .authenticate_with_password(&mut rng, &clock, session, &user_password)
        .await?;

    let reply = render(&mut rng, &clock, templates.clone(), session, cookie_jar).await?;

    repo.save().await?;

    Ok(reply)
}
