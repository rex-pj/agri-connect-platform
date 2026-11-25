//! Giving a record (a business, a partner) an account that signs in to it,
//! shared so every "assign an account" form follows the same rules.

use domain::{
    models::user::NewUser, repositories::user_repository_trait::UserRepositoryTrait, Error,
    ErrorCode, Result, Validation,
};

use crate::{
    ports::PasswordHasher,
    validation::{validate_email, validate_password},
};

/// The account `email` names if it has `role` (its password is left alone),
/// else a new account of that role with that email and `password`; its id
/// and the email as stored. An account of another role is refused, so an
/// admin can never be turned into something else here; so is one whose
/// address was never confirmed: anyone may sign up with a stranger's
/// address, and the record would go to whoever did.
pub(crate) async fn account_for_role(
    user_repo: &(dyn UserRepositoryTrait + Send + Sync),
    hasher: &dyn PasswordHasher,
    email: &str,
    password: &str,
    role: &str,
) -> Result<(i32, String)> {
    let email = email.trim().to_lowercase();
    let mut rules = Validation::new();
    rules.rule("email", validate_email(&email));
    rules.finish()?;
    let existing = user_repo.find_account_by_email(&email).await?;
    if existing.is_none() {
        Validation::new()
            .rule("password", validate_password(password))
            .finish()?;
    }

    let id = match existing {
        Some(u) if u.role != role => {
            return Err(Error::field("email", ErrorCode::EmailTakenByOtherRole));
        }
        Some(u) if !u.email_verified => {
            return Err(Error::field("email", ErrorCode::AccountEmailUnconfirmed));
        }
        Some(u) => u.id,
        None => {
            let password_hash = hasher.hash(password).await?;
            user_repo
                .create(NewUser {
                    email: email.clone(),
                    password_hash,
                    role: role.into(),
                    full_name: None,
                    email_verified: true,
                })
                .await?
        }
    };
    Ok((id, email))
}
