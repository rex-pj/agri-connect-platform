//! The one error type every layer returns.
//!
//! An error is a machine `code` plus optional parameters and per-field codes.
//! It never carries user-facing text: the web layer resolves text from the
//! code through the locale files, so a code that is missing there fails the
//! locale test instead of shipping an English string to farmers.

use std::fmt;

/// HTTP-shaped classification. The web layer maps it to a status code; nothing
/// else in the codebase should branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Locked,
    TooManyRequests,
    Internal,
}

macro_rules! error_codes {
    ($( $variant:ident => ($kind:ident, $key:literal) ),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum ErrorCode { $($variant),* }

        impl ErrorCode {
            /// Every code, so tests can check each one has locale text.
            pub const ALL: &'static [ErrorCode] = &[$(ErrorCode::$variant),*];

            /// Stable wire identifier. Clients may branch on it, so renaming
            /// one is a breaking API change.
            pub const fn as_str(self) -> &'static str {
                match self { $(ErrorCode::$variant => $key),* }
            }

            pub const fn kind(self) -> ErrorKind {
                match self { $(ErrorCode::$variant => ErrorKind::$kind),* }
            }
        }
    };
}

error_codes! {
    // ── Generic ──────────────────────────────────────────────────────────────
    Internal                  => (Internal,        "internal"),
    RouteNotFound             => (NotFound,        "route_not_found"),
    ValidationFailed          => (BadRequest,      "validation_failed"),
    InvalidRequestBody        => (BadRequest,      "invalid_request_body"),
    InvalidPathParam          => (BadRequest,      "invalid_path_param"),
    InvalidQuery              => (BadRequest,      "invalid_query"),
    TooManyRequests           => (TooManyRequests, "too_many_requests"),

    // ── Authentication / access ──────────────────────────────────────────────
    AuthenticationRequired    => (Unauthorized,    "authentication_required"),
    InvalidToken              => (Unauthorized,    "invalid_token"),
    InvalidCredentials        => (Unauthorized,    "invalid_credentials"),
    CurrentPasswordIncorrect  => (Unauthorized,    "current_password_incorrect"),
    AccountLocked             => (Locked,          "account_locked"),
    AccountDeactivated        => (Forbidden,       "account_deactivated"),
    AccountHasNoRole          => (Forbidden,       "account_has_no_role"),
    PermissionDenied          => (Forbidden,       "permission_denied"),
    AdminAlreadyExists        => (Forbidden,       "admin_already_exists"),
    ResetTokenInvalid         => (BadRequest,      "reset_token_invalid"),

    // ── Field rules ──────────────────────────────────────────────────────────
    UsernameTooShort          => (BadRequest,      "username_too_short"),
    PasswordTooShort          => (BadRequest,      "password_too_short"),
    PasswordTooLong           => (BadRequest,      "password_too_long"),
    InvalidPhone              => (BadRequest,      "invalid_phone"),
    InvalidCoordinates        => (BadRequest,      "invalid_coordinates"),
    RadiusOutOfRange          => (BadRequest,      "radius_out_of_range"),
    NameRequired              => (BadRequest,      "name_required"),
    SlugRequired              => (BadRequest,      "slug_required"),
    LabelRequired             => (BadRequest,      "label_required"),
    BusinessNameRequired      => (BadRequest,      "business_name_required"),
    RequesterNameRequired     => (BadRequest,      "requester_name_required"),
    RequesterNameTooLong      => (BadRequest,      "requester_name_too_long"),
    TopicRequired             => (BadRequest,      "topic_required"),
    TopicTooLong              => (BadRequest,      "topic_too_long"),
    NoteTooLong               => (BadRequest,      "note_too_long"),
    InvalidPrice              => (BadRequest,      "invalid_price"),
    InvalidRole               => (BadRequest,      "invalid_role"),
    InvalidCategoryType       => (BadRequest,      "invalid_category_type"),
    InvalidInquiryStatus      => (BadRequest,      "invalid_inquiry_status"),
    InvalidProductCategory    => (BadRequest,      "invalid_product_category"),
    InvalidServiceCategory    => (BadRequest,      "invalid_service_category"),

    // ── Conflicts ────────────────────────────────────────────────────────────
    UsernameTaken             => (Conflict,        "username_taken"),
    UsernameTakenByOtherRole  => (Conflict,        "username_taken_by_other_role"),
    EmailTaken                => (Conflict,        "email_taken"),
    DemoAlreadyLoaded         => (Conflict,        "demo_already_loaded"),

    // ── Not found ────────────────────────────────────────────────────────────
    AccountNotFound           => (NotFound,        "account_not_found"),
    AccountNotLinkedToBusiness=> (NotFound,        "account_not_linked_to_business"),
    UserNotFound              => (NotFound,        "user_not_found"),
    RoleNotFound              => (NotFound,        "role_not_found"),
    ProviderNotFound          => (NotFound,        "provider_not_found"),
    ProductNotFound           => (NotFound,        "product_not_found"),
    ServiceNotFound           => (NotFound,        "service_not_found"),
    CategoryNotFound          => (NotFound,        "category_not_found"),
    LocationNotFound          => (NotFound,        "location_not_found"),
    InquiryNotFound           => (NotFound,        "inquiry_not_found"),
    ReviewNotFound            => (NotFound,        "review_not_found"),
}

/// One field that failed validation. Parameters belong to the field: two rules
/// can both use `{min}` with different values (username 3, password 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    pub field:  &'static str,
    pub code:   ErrorCode,
    pub params: Vec<(&'static str, String)>,
}

#[derive(Debug, Clone)]
pub struct Error {
    code:   ErrorCode,
    params: Vec<(&'static str, String)>,
    fields: Vec<FieldError>,
    /// Context for the server log. Never sent to a client: it may contain SQL
    /// errors or other internals.
    detail: Option<String>,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn new(code: ErrorCode) -> Self {
        Self { code, params: Vec::new(), fields: Vec::new(), detail: None }
    }

    /// An unexpected failure. `detail` is logged, never shown to users.
    pub fn internal(detail: impl Into<String>) -> Self {
        Self { detail: Some(detail.into()), ..Self::new(ErrorCode::Internal) }
    }

    /// A value the locale text interpolates, e.g. `{min}` in "at least {min} characters".
    pub fn with_param(mut self, name: &'static str, value: impl ToString) -> Self {
        self.params.push((name, value.to_string()));
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn kind(&self) -> ErrorKind {
        self.code.kind()
    }

    pub fn params(&self) -> &[(&'static str, String)] {
        &self.params
    }

    pub fn fields(&self) -> &[FieldError] {
        &self.fields
    }

    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code.as_str())?;
        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

impl From<ErrorCode> for Error {
    fn from(code: ErrorCode) -> Self {
        Self::new(code)
    }
}

/// Collects field errors so a form gets every problem back at once instead of
/// one per round trip.
#[derive(Debug, Default)]
pub struct Validation {
    fields: Vec<FieldError>,
}

impl Validation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `code` against `field` when `failed` is true. Only the first
    /// failure per field is kept: the form shows one message per input.
    pub fn check(&mut self, failed: bool, field: &'static str, code: ErrorCode) -> &mut Self {
        if failed && !self.fields.iter().any(|f| f.field == field) {
            self.fields.push(FieldError { field, code, params: Vec::new() });
        }
        self
    }

    /// Like `check`, for a code whose text needs a parameter (e.g. a limit).
    pub fn check_with(
        &mut self,
        failed: bool,
        field: &'static str,
        code: ErrorCode,
        param: (&'static str, usize),
    ) -> &mut Self {
        if failed && !self.fields.iter().any(|f| f.field == field) {
            self.fields.push(FieldError { field, code, params: vec![(param.0, param.1.to_string())] });
        }
        self
    }

    /// Records a field error from a rule that returns `Result`, keeping the
    /// rule's parameters for interpolation.
    pub fn rule(&mut self, field: &'static str, result: Result<()>) -> &mut Self {
        if let Err(e) = result {
            if !self.fields.iter().any(|f| f.field == field) {
                self.fields.push(FieldError { field, code: e.code, params: e.params });
            }
        }
        self
    }

    pub fn finish(&mut self) -> Result<()> {
        if self.fields.is_empty() {
            return Ok(());
        }
        Err(Error {
            code:   ErrorCode::ValidationFailed,
            params: Vec::new(),
            fields: std::mem::take(&mut self.fields),
            detail: None,
        })
    }
}

#[cfg(feature = "axum")]
mod http {
    //! Status-code mapping for axum. The body is written later by the web
    //! layer's error renderer, which has the locale; this impl only parks the
    //! error in the response extensions. A route mounted outside that renderer
    //! would answer with an empty body, which the web crate's tests guard.

    use axum::{
        extract::rejection::{JsonRejection, PathRejection, QueryRejection},
        http::StatusCode,
        response::{IntoResponse, Response},
    };

    use super::{Error, ErrorCode, ErrorKind};

    impl ErrorKind {
        pub fn status(self) -> StatusCode {
            match self {
                ErrorKind::BadRequest      => StatusCode::BAD_REQUEST,
                ErrorKind::Unauthorized    => StatusCode::UNAUTHORIZED,
                ErrorKind::Forbidden       => StatusCode::FORBIDDEN,
                ErrorKind::NotFound        => StatusCode::NOT_FOUND,
                ErrorKind::Conflict        => StatusCode::CONFLICT,
                ErrorKind::Locked          => StatusCode::LOCKED,
                ErrorKind::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
                ErrorKind::Internal        => StatusCode::INTERNAL_SERVER_ERROR,
            }
        }
    }

    impl IntoResponse for Error {
        fn into_response(self) -> Response {
            let mut response = self.kind().status().into_response();
            response.extensions_mut().insert(self);
            response
        }
    }

    impl From<JsonRejection> for Error {
        fn from(rejection: JsonRejection) -> Self {
            Error::new(ErrorCode::InvalidRequestBody).with_detail(rejection.body_text())
        }
    }

    impl From<PathRejection> for Error {
        fn from(rejection: PathRejection) -> Self {
            Error::new(ErrorCode::InvalidPathParam).with_detail(rejection.body_text())
        }
    }

    impl From<QueryRejection> for Error {
        fn from(rejection: QueryRejection) -> Self {
            Error::new(ErrorCode::InvalidQuery).with_detail(rejection.body_text())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_codes_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for code in ErrorCode::ALL {
            assert!(seen.insert(code.as_str()), "duplicate wire code {}", code.as_str());
        }
    }

    #[test]
    fn validation_keeps_first_error_per_field() {
        let err = Validation::new()
            .check(true, "phone", ErrorCode::InvalidPhone)
            .check(true, "phone", ErrorCode::NameRequired)
            .check(false, "name", ErrorCode::NameRequired)
            .finish()
            .err();
        let fields = err.as_ref().map(Error::fields).unwrap_or_default();
        assert_eq!(fields, [FieldError { field: "phone", code: ErrorCode::InvalidPhone, params: vec![] }]);
    }

    #[test]
    fn each_field_keeps_its_own_params() {
        // Username and password both use `{min}`; merging them made the
        // password message say "at least 3".
        let err = Validation::new()
            .rule("username", Err(Error::new(ErrorCode::UsernameTooShort).with_param("min", 3)))
            .rule("password", Err(Error::new(ErrorCode::PasswordTooShort).with_param("min", 8)))
            .finish()
            .err();
        let params: Vec<_> = err.iter().flat_map(|e| e.fields()).map(|f| f.params.clone()).collect();
        assert_eq!(params, [vec![("min", "3".to_owned())], vec![("min", "8".to_owned())]]);
    }

    #[test]
    fn validation_without_failures_is_ok() {
        assert!(Validation::new().check(false, "x", ErrorCode::NameRequired).finish().is_ok());
    }
}
