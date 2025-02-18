// Copyright 2022, 2023 The Matrix.org Foundation C.I.C.
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

use async_trait::async_trait;
use mas_data_model::{User, UserEmail, UserEmailVerification};
use rand_core::RngCore;
use ulid::Ulid;

use crate::{pagination::Page, repository_impl, Clock, Pagination};

/// A [`UserEmailRepository`] helps interacting with [`UserEmail`] saved in the
/// storage backend
#[async_trait]
pub trait UserEmailRepository: Send + Sync {
    /// The error type returned by the repository
    type Error;

    /// Lookup an [`UserEmail`] by its ID
    ///
    /// Returns `None` if no [`UserEmail`] was found
    ///
    /// # Parameters
    ///
    /// * `id`: The ID of the [`UserEmail`] to lookup
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn lookup(&mut self, id: Ulid) -> Result<Option<UserEmail>, Self::Error>;

    /// Lookup an [`UserEmail`] by its email address for a [`User`]
    ///
    /// Returns `None` if no matching [`UserEmail`] was found
    ///
    /// # Parameters
    ///
    /// * `user`: The [`User`] for whom to lookup the [`UserEmail`]
    /// * `email`: The email address to lookup
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn find(&mut self, user: &User, email: &str) -> Result<Option<UserEmail>, Self::Error>;

    /// Get the primary [`UserEmail`] of a [`User`]
    ///
    /// Returns `None` if no the user has no primary [`UserEmail`]
    ///
    /// # Parameters
    ///
    /// * `user`: The [`User`] for whom to lookup the primary [`UserEmail`]
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn get_primary(&mut self, user: &User) -> Result<Option<UserEmail>, Self::Error>;

    /// Get all [`UserEmail`] of a [`User`]
    ///
    /// # Parameters
    ///
    /// * `user`: The [`User`] for whom to lookup the [`UserEmail`]
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn all(&mut self, user: &User) -> Result<Vec<UserEmail>, Self::Error>;

    /// List [`UserEmail`] of a [`User`] with the given pagination
    ///
    /// # Parameters
    ///
    /// * `user`: The [`User`] for whom to lookup the [`UserEmail`]
    /// * `pagination`: The pagination parameters
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn list_paginated(
        &mut self,
        user: &User,
        pagination: Pagination,
    ) -> Result<Page<UserEmail>, Self::Error>;

    /// Count the [`UserEmail`] of a [`User`]
    ///
    /// # Parameters
    ///
    /// * `user`: The [`User`] for whom to count the [`UserEmail`]
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn count(&mut self, user: &User) -> Result<usize, Self::Error>;

    /// Create a new [`UserEmail`] for a [`User`]
    ///
    /// Returns the newly created [`UserEmail`]
    ///
    /// # Parameters
    ///
    /// * `rng`: The random number generator to use
    /// * `clock`: The clock to use
    /// * `user`: The [`User`] for whom to create the [`UserEmail`]
    /// * `email`: The email address of the [`UserEmail`]
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn add(
        &mut self,
        rng: &mut (dyn RngCore + Send),
        clock: &dyn Clock,
        user: &User,
        email: String,
    ) -> Result<UserEmail, Self::Error>;

    /// Delete a [`UserEmail`]
    ///
    /// # Parameters
    ///
    /// * `user_email`: The [`UserEmail`] to delete
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn remove(&mut self, user_email: UserEmail) -> Result<(), Self::Error>;

    /// Mark a [`UserEmail`] as verified
    ///
    /// Returns the updated [`UserEmail`]
    ///
    /// # Parameters
    ///
    /// * `clock`: The clock to use
    /// * `user_email`: The [`UserEmail`] to mark as verified
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn mark_as_verified(
        &mut self,
        clock: &dyn Clock,
        user_email: UserEmail,
    ) -> Result<UserEmail, Self::Error>;

    /// Mark a [`UserEmail`] as primary
    ///
    /// # Parameters
    ///
    /// * `user_email`: The [`UserEmail`] to mark as primary
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn set_as_primary(&mut self, user_email: &UserEmail) -> Result<(), Self::Error>;

    /// Add a [`UserEmailVerification`] for a [`UserEmail`]
    ///
    /// # Parameters
    ///
    /// * `rng`: The random number generator to use
    /// * `clock`: The clock to use
    /// * `user_email`: The [`UserEmail`] for which to add the
    ///   [`UserEmailVerification`]
    /// * `max_age`: The duration for which the [`UserEmailVerification`] is
    ///   valid
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn add_verification_code(
        &mut self,
        rng: &mut (dyn RngCore + Send),
        clock: &dyn Clock,
        user_email: &UserEmail,
        max_age: chrono::Duration,
        code: String,
    ) -> Result<UserEmailVerification, Self::Error>;

    /// Find a [`UserEmailVerification`] for a [`UserEmail`] by its code
    ///
    /// Returns `None` if no matching [`UserEmailVerification`] was found
    ///
    /// # Parameters
    ///
    /// * `clock`: The clock to use
    /// * `user_email`: The [`UserEmail`] for which to lookup the
    ///   [`UserEmailVerification`]
    /// * `code`: The code used to lookup
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn find_verification_code(
        &mut self,
        clock: &dyn Clock,
        user_email: &UserEmail,
        code: &str,
    ) -> Result<Option<UserEmailVerification>, Self::Error>;

    /// Consume a [`UserEmailVerification`]
    ///
    /// Returns the consumed [`UserEmailVerification`]
    ///
    /// # Parameters
    ///
    /// * `clock`: The clock to use
    /// * `verification`: The [`UserEmailVerification`] to consume
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the underlying repository fails
    async fn consume_verification_code(
        &mut self,
        clock: &dyn Clock,
        verification: UserEmailVerification,
    ) -> Result<UserEmailVerification, Self::Error>;
}

repository_impl!(UserEmailRepository:
    async fn lookup(&mut self, id: Ulid) -> Result<Option<UserEmail>, Self::Error>;
    async fn find(&mut self, user: &User, email: &str) -> Result<Option<UserEmail>, Self::Error>;
    async fn get_primary(&mut self, user: &User) -> Result<Option<UserEmail>, Self::Error>;

    async fn all(&mut self, user: &User) -> Result<Vec<UserEmail>, Self::Error>;
    async fn list_paginated(
        &mut self,
        user: &User,
        pagination: Pagination,
    ) -> Result<Page<UserEmail>, Self::Error>;
    async fn count(&mut self, user: &User) -> Result<usize, Self::Error>;

    async fn add(
        &mut self,
        rng: &mut (dyn RngCore + Send),
        clock: &dyn Clock,
        user: &User,
        email: String,
    ) -> Result<UserEmail, Self::Error>;
    async fn remove(&mut self, user_email: UserEmail) -> Result<(), Self::Error>;

    async fn mark_as_verified(
        &mut self,
        clock: &dyn Clock,
        user_email: UserEmail,
    ) -> Result<UserEmail, Self::Error>;

    async fn set_as_primary(&mut self, user_email: &UserEmail) -> Result<(), Self::Error>;

    async fn add_verification_code(
        &mut self,
        rng: &mut (dyn RngCore + Send),
        clock: &dyn Clock,
        user_email: &UserEmail,
        max_age: chrono::Duration,
        code: String,
    ) -> Result<UserEmailVerification, Self::Error>;

    async fn find_verification_code(
        &mut self,
        clock: &dyn Clock,
        user_email: &UserEmail,
        code: &str,
    ) -> Result<Option<UserEmailVerification>, Self::Error>;

    async fn consume_verification_code(
        &mut self,
        clock: &dyn Clock,
        verification: UserEmailVerification,
    ) -> Result<UserEmailVerification, Self::Error>;
);
