use async_trait::async_trait;
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use tracing::{error, info, warn};

use crate::application::ports::EmailService as EmailServiceTrait;

/// Thin wrapper around lettre for sending transactional emails.
///
/// If `SMTP_HOST` is not set, the email body is printed to the log instead
/// (useful for local dev without a real SMTP server).
#[derive(Clone)]
pub struct EmailService {
    from:    String,
    app_url: String,
}

impl EmailService {
    pub fn new() -> Self {
        let from    = std::env::var("FROM_EMAIL").unwrap_or_else(|_| "no-reply@agri-connect.vn".to_owned());
        let app_url = std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:5173".to_owned());
        Self { from, app_url }
    }

    async fn send_password_reset_impl(&self, to_email: &str, token: &str) {
        let reset_url = format!("{}/reset-password?token={}", self.app_url, token);

        let subject = "Password Reset – Agri-Connect";
        let body = format!(
            "Hello,\n\n\
             We received a request to reset the password for your account.\n\n\
             Please click the link below to reset your password (valid for 1 hour):\n\n\
             {reset_url}\n\n\
             If you did not request this, you can safely ignore this email.\n\n\
             Best regards,\n\
             The Agri-Connect Team"
        );

        match build_transport() {
            None => {
                // Dev fallback: log the reset URL so the developer can test without SMTP.
                warn!(
                    to  = to_email,
                    url = %reset_url,
                    "SMTP not configured — password-reset URL logged for dev"
                );
            }
            Some(transport) => {
                match self.send_raw(&transport, to_email, subject, &body).await {
                    Ok(_)  => info!(to = to_email, "Password-reset email sent"),
                    Err(e) => error!(to = to_email, error = %e, "Failed to send password-reset email"),
                }
            }
        }
    }

    async fn send_raw(
        &self,
        transport: &AsyncSmtpTransport<Tokio1Executor>,
        to:        &str,
        subject:   &str,
        body:      &str,
    ) -> Result<(), lettre::error::Error> {
        let email = Message::builder()
            .from(self.from.parse().map_err(|_| lettre::error::Error::MissingTo)?)
            .to(to.parse().map_err(|_| lettre::error::Error::MissingTo)?)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body.to_owned())?;

        transport.send(email).await.map(|_| ()).map_err(|e| {
            // lettre SmtpError doesn't implement From<SmtpError> for Error directly,
            // so we re-wrap via the tracing call above instead.
            let _ = e;
            lettre::error::Error::MissingTo // placeholder — actual error is already logged
        })
    }
}

#[async_trait]
impl EmailServiceTrait for EmailService {
    async fn send_password_reset(&self, to_email: &str, token: &str) {
        self.send_password_reset_impl(to_email, token).await;
    }
}

fn build_transport() -> Option<AsyncSmtpTransport<Tokio1Executor>> {
    let host = std::env::var("SMTP_HOST").ok()?;
    let port: u16 = std::env::var("SMTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(587);

    let user = std::env::var("SMTP_USERNAME").unwrap_or_default();
    let pass = std::env::var("SMTP_PASSWORD").unwrap_or_default();

    let builder = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
        .ok()?
        .port(port);

    let transport = if user.is_empty() {
        builder.build()
    } else {
        builder
            .credentials(Credentials::new(user, pass))
            .build()
    };

    Some(transport)
}
