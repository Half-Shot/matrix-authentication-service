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

use chrono::{DateTime, Utc};
use oauth2_types::scope::Scope;
use serde::Serialize;
use ulid::Ulid;

use crate::InvalidTransitionError;

trait T {
    type State;
}

impl T for Session {
    type State = SessionState;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub enum SessionState {
    #[default]
    Valid,
    Finished {
        finished_at: DateTime<Utc>,
    },
}

impl SessionState {
    /// Returns `true` if the session state is [`Valid`].
    ///
    /// [`Valid`]: SessionState::Valid
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Returns `true` if the session state is [`Finished`].
    ///
    /// [`Finished`]: SessionState::Finished
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Finished { .. })
    }

    pub fn finish(self, finished_at: DateTime<Utc>) -> Result<Self, InvalidTransitionError> {
        match self {
            Self::Valid => Ok(Self::Finished { finished_at }),
            Self::Finished { .. } => Err(InvalidTransitionError),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Session {
    pub id: Ulid,
    pub state: SessionState,
    pub created_at: DateTime<Utc>,
    pub user_session_id: Ulid,
    pub client_id: Ulid,
    pub scope: Scope,
}

impl std::ops::Deref for Session {
    type Target = SessionState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Session {
    pub fn finish(mut self, finished_at: DateTime<Utc>) -> Result<Self, InvalidTransitionError> {
        self.state = self.state.finish(finished_at)?;
        Ok(self)
    }
}
