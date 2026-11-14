//! Giving a record (a business, a partner) an account that signs in to it,
//! shared so every "assign an account" form follows the same rules.

use domain::{
    models::user::NewUser, repositories::user_repository_trait::UserRepositoryTrait, Error,
    ErrorCode, Result, Validation,
};

use crate::{
    ports::PasswordHasher,
    validation::{validate_email, validate_password, validate_username},
};

/// The account `username` names if it has `role` (its email and password are
/// left alone), else a new account of that role with the given email and
/// password; its id and the username as stored. An account of another role
/// is refused, so an admin can never be turned into something else here.
pub(crate) async fn account_for_role(
    user_repo: &(dyn UserRepositoryTrait + Send + Sync),
    hasher: &dyn PasswordHasher,
    username: &str,
    email: &str,
    password: &str,
    role: &str,
) -> Result<(i32, String)> {
    let username = username.trim().to_lowercase();
    let email = email.trim().to_lowercase();
    let existing = user_repo.find_by_username(&username).await?;
    let mut rules = Validation::new();
    rules.rule("username", validate_username(&username));
    if existing.is_none() {
        rules
            .rule("email", validate_email(&email))
            .rule("password", validate_password(password));
    }
    rules.finish()?;

    let id = match existing {
        Some(u) if u.role != role => {
            return Err(Error::field(
                "username",
                ErrorCode::UsernameTakenByOtherRole,
            ));
        }
        Some(u) => u.id,
        None => {
            if user_repo.email_exists(&email).await? {
                return Err(Error::field("email", ErrorCode::EmailTaken));
            }
            let password_hash = hasher.hash(password).await?;
            user_repo
                .create(NewUser {
                    username: username.clone(),
                    email,
                    password_hash,
                    role: role.into(),
                    full_name: None,
                    email_verified: true,
                })
                .await?
        }
    };
    Ok((id, username))
}
