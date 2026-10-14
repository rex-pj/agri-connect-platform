//! `EmailService` adapters. `startup.rs` picks one: SMTP when `SMTP_HOST` is
//! set, otherwise the log fallback, announced with a WARN at startup.

use application::{constants::PASSWORD_RESET_EXPIRY_HOURS, ports::EmailService};
use async_trait::async_trait;
use domain::{Error, Result};
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use tracing::{error, info, warn};

fn reset_url(app_url: &str, token: &str) -> String {
    format!("{}/reset-password?token={token}", app_url.trim_end_matches('/'))
}

pub struct SmtpConfig {
    pub host:     String,
    pub port:     u16,
    /// Both or neither: an unauthenticated relay is allowed for local MTAs.
    pub credentials: Option<(String, String)>,
    pub from:     String,
    pub app_url:  String,
}

pub struct SmtpEmailService {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from:      Mailbox,
    app_url:   String,
}

impl SmtpEmailService {
    pub fn new(config: SmtpConfig) -> Result<Self> {
        let from: Mailbox = config
            .from
            .parse()
            .map_err(|e| Error::internal(format!("FROM_EMAIL is not a valid address: {e}")))?;
        let mut builder = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map_err(|e| Error::internal(format!("SMTP_HOST is not usable: {e}")))?
            .port(config.port);
        if let Some((user, pass)) = config.credentials {
            builder = builder.credentials(Credentials::new(user, pass));
        }
        Ok(Self { transport: builder.build(), from, app_url: config.app_url })
    }

    async fn send(&self, to: &str, subject: &str, body: String) -> std::result::Result<(), String> {
        let to: Mailbox = to.parse().map_err(|e| format!("bad recipient: {e}"))?;
        let message = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body)
            .map_err(|e| format!("build: {e}"))?;
        self.transport.send(message).await.map(|_| ()).map_err(|e| format!("smtp: {e}"))
    }
}

#[async_trait]
impl EmailService for SmtpEmailService {
    async fn send_password_reset(&self, to_email: &str, token: &str) {
        let url = reset_url(&self.app_url, token);
        // Plain text with no user-supplied content: nothing here can be used to
        // inject markup into the message.
        let body = format!(
            "Xin chào,\n\n\
             Chúng tôi nhận được yêu cầu đặt lại mật khẩu cho tài khoản của bạn.\n\
             Mở liên kết sau để đặt mật khẩu mới (hiệu lực {PASSWORD_RESET_EXPIRY_HOURS} giờ):\n\n\
             {url}\n\n\
             Nếu bạn không yêu cầu, hãy bỏ qua email này.\n\n\
             Agri-Connect"
        );
        match self.send(to_email, "Đặt lại mật khẩu – Agri-Connect", body).await {
            Ok(()) => info!(to = to_email, "password-reset email sent"),
            Err(e) => error!(to = to_email, error = %e, "password-reset email failed"),
        }
    }
}

/// Development fallback: writes the reset link to the log instead of sending
/// it. Never select it in production; anyone with log access could take over
/// accounts with the links it prints.
pub struct LogEmailService {
    app_url: String,
}

impl LogEmailService {
    pub fn new(app_url: String) -> Self {
        Self { app_url }
    }
}

#[async_trait]
impl EmailService for LogEmailService {
    async fn send_password_reset(&self, to_email: &str, token: &str) {
        warn!(to = to_email, url = %reset_url(&self.app_url, token), "SMTP not configured; password-reset link logged instead of sent");
    }
}

#[cfg(test)]
mod tests {
    use super::reset_url;

    #[test]
    fn reset_url_does_not_double_the_slash() {
        assert_eq!(reset_url("https://a.vn/", "t"), "https://a.vn/reset-password?token=t");
        assert_eq!(reset_url("https://a.vn", "t"), "https://a.vn/reset-password?token=t");
    }
}
