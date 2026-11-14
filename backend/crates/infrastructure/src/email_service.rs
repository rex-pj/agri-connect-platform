//! `EmailService` adapters. `startup.rs` picks one: SMTP when `SMTP_HOST` is
//! set, otherwise the log fallback, announced with a WARN at startup.

use application::{
    constants::{EMAIL_VERIFICATION_EXPIRY_HOURS, PASSWORD_RESET_EXPIRY_HOURS},
    ports::{EmailService, InquiryNotice, JobNotice},
};
use async_trait::async_trait;
use domain::{Error, Result};
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use tracing::{error, info, warn};

/// `path` (starting with `/`) on the public origin, without a doubled slash.
fn site_url(app_url: &str, path: &str) -> String {
    format!("{}{path}", app_url.trim_end_matches('/'))
}

fn reset_url(app_url: &str, token: &str) -> String {
    site_url(app_url, &format!("/reset-password?token={token}"))
}

fn verify_url(app_url: &str, token: &str) -> String {
    site_url(app_url, &format!("/verify-email?token={token}"))
}

/// The wording of every email, from the locale file (`[text.email]`), with
/// `{name}` placeholders; `startup.rs` reads it there and passes it in.
#[derive(Debug, Clone)]
pub struct EmailTexts {
    /// Signs every email and ends every subject.
    pub site_name: String,
    /// `{site}`.
    pub reset_subject: String,
    /// `{url}`, `{hours}`, `{site}`.
    pub reset_body: String,
    /// `{site}`.
    pub verify_subject: String,
    /// `{url}`, `{hours}`, `{site}`.
    pub verify_body: String,
    /// `{site}`.
    pub inquiry_subject: String,
    /// `{business}`, `{name}`, `{phone}`, `{topic}`, `{note}`, `{inbox}`,
    /// `{site}`.
    pub inquiry_body: String,
    /// `{site}`.
    pub job_subject: String,
    /// `{business}`, `{category}`, `{area}`, `{description}`, `{name}`,
    /// `{phone}`, `{jobs}`, `{site}`.
    pub job_body: String,
    /// `{note}`: the line `{note}` becomes when the farmer left one.
    pub inquiry_note: String,
}

/// `template` with each `{name}` replaced by its value.
fn fill(template: &str, params: &[(&str, &str)]) -> String {
    params
        .iter()
        .fold(template.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
}

/// The provider's "new inquiry" email. Plain text: the farmer's name, topic
/// and note are shown as typed, never interpreted.
fn inquiry_body(texts: &EmailTexts, app_url: &str, n: &InquiryNotice) -> String {
    let note = n
        .note
        .as_deref()
        .map(|note| fill(&texts.inquiry_note, &[("note", note)]))
        .unwrap_or_default();
    fill(
        &texts.inquiry_body,
        &[
            ("business", &n.business_name),
            ("name", &n.requester_name),
            ("phone", &n.requester_phone),
            ("topic", &n.topic),
            ("note", &note),
            ("inbox", &site_url(app_url, &n.inbox_path)),
            ("site", &texts.site_name),
        ],
    )
}

/// The provider's "a job near you" email. Plain text: the farmer's name and
/// description are shown as typed, never interpreted. No area: blank.
fn job_body(texts: &EmailTexts, app_url: &str, n: &JobNotice) -> String {
    fill(
        &texts.job_body,
        &[
            ("business", &n.business_name),
            ("category", &n.category),
            ("area", n.area.as_deref().unwrap_or_default()),
            ("description", &n.description),
            ("name", &n.requester_name),
            ("phone", &n.requester_phone),
            ("jobs", &site_url(app_url, &n.jobs_path)),
            ("site", &texts.site_name),
        ],
    )
}

pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    /// Both or neither: an unauthenticated relay is allowed for local MTAs.
    pub credentials: Option<(String, String)>,
    pub from: String,
    pub app_url: String,
    pub texts: EmailTexts,
}

pub struct SmtpEmailService {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    app_url: String,
    texts: EmailTexts,
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
        Ok(Self {
            transport: builder.build(),
            from,
            app_url: config.app_url,
            texts: config.texts,
        })
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
        self.transport
            .send(message)
            .await
            .map(|_| ())
            .map_err(|e| format!("smtp: {e}"))
    }
}

#[async_trait]
impl EmailService for SmtpEmailService {
    async fn send_password_reset(&self, to_email: &str, token: &str) {
        let url = reset_url(&self.app_url, token);
        let site = self.texts.site_name.as_str();
        // Plain text with no user-supplied content: nothing here can be used to
        // inject markup into the message.
        let body = fill(
            &self.texts.reset_body,
            &[
                ("url", &url),
                ("hours", &PASSWORD_RESET_EXPIRY_HOURS.to_string()),
                ("site", site),
            ],
        );
        let subject = fill(&self.texts.reset_subject, &[("site", site)]);
        match self.send(to_email, &subject, body).await {
            Ok(()) => info!(to = to_email, "password-reset email sent"),
            Err(e) => error!(to = to_email, error = %e, "password-reset email failed"),
        }
    }

    async fn send_email_verification(&self, to_email: &str, token: &str) {
        let url = verify_url(&self.app_url, token);
        let site = self.texts.site_name.as_str();
        // Plain text with no user-supplied content, as the reset email.
        let body = fill(
            &self.texts.verify_body,
            &[
                ("url", &url),
                ("hours", &EMAIL_VERIFICATION_EXPIRY_HOURS.to_string()),
                ("site", site),
            ],
        );
        let subject = fill(&self.texts.verify_subject, &[("site", site)]);
        match self.send(to_email, &subject, body).await {
            Ok(()) => info!(to = to_email, "email-confirmation email sent"),
            Err(e) => error!(to = to_email, error = %e, "email-confirmation email failed"),
        }
    }

    async fn send_new_inquiry(&self, to_email: &str, notice: &InquiryNotice) {
        let body = inquiry_body(&self.texts, &self.app_url, notice);
        let subject = fill(
            &self.texts.inquiry_subject,
            &[("site", &self.texts.site_name)],
        );
        match self.send(to_email, &subject, body).await {
            Ok(()) => info!(to = to_email, "new-inquiry email sent"),
            Err(e) => error!(to = to_email, error = %e, "new-inquiry email failed"),
        }
    }

    async fn send_new_job(&self, to_email: &str, notice: &JobNotice) {
        let body = job_body(&self.texts, &self.app_url, notice);
        let subject = fill(&self.texts.job_subject, &[("site", &self.texts.site_name)]);
        match self.send(to_email, &subject, body).await {
            Ok(()) => info!(to = to_email, "new-job email sent"),
            Err(e) => error!(to = to_email, error = %e, "new-job email failed"),
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

    async fn send_email_verification(&self, to_email: &str, token: &str) {
        warn!(to = to_email, url = %verify_url(&self.app_url, token), "SMTP not configured; email-confirmation link logged instead of sent");
    }

    async fn send_new_inquiry(&self, to_email: &str, notice: &InquiryNotice) {
        // The farmer's phone number stays out of the log.
        warn!(
            to = to_email,
            inbox = %site_url(&self.app_url, &notice.inbox_path),
            "SMTP not configured; new-inquiry email not sent"
        );
    }

    async fn send_new_job(&self, to_email: &str, notice: &JobNotice) {
        // The farmer's phone number stays out of the log.
        warn!(
            to = to_email,
            jobs = %site_url(&self.app_url, &notice.jobs_path),
            "SMTP not configured; new-job email not sent"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{inquiry_body, job_body, reset_url, EmailTexts, InquiryNotice, JobNotice};

    fn texts() -> EmailTexts {
        EmailTexts {
            site_name: "Site".into(),
            reset_subject: "Reset - {site}".into(),
            reset_body: "{url} ({hours} h) {site}".into(),
            verify_subject: "Confirm - {site}".into(),
            verify_body: "{url} ({hours} h) {site}".into(),
            inquiry_subject: "Call back - {site}".into(),
            inquiry_body:
                "Hi {business}\nName: {name}\nPhone: {phone}\nNeeds: {topic}\n{note}{inbox}\n{site}"
                    .into(),
            inquiry_note: "Note: {note}\n".into(),
            job_subject: "Job - {site}".into(),
            job_body:
                "Hi {business}\n{category} in {area}: {description}\n{name} {phone}\n{jobs}\n{site}"
                    .into(),
        }
    }

    #[test]
    fn the_job_email_has_the_job_who_to_call_and_the_jobs_link() {
        let notice = JobNotice {
            business_name: "Drone Út".into(),
            category: "Phun thuốc".into(),
            area: None,
            description: "3 công lúa".into(),
            requester_name: "Chú Ba".into(),
            requester_phone: "0912345678".into(),
            jobs_path: "/provider/jobs".into(),
        };
        let body = job_body(&texts(), "https://agri.vn", &notice);
        assert!(body.contains("Phun thuốc in : 3 công lúa"), "{body}");
        assert!(body.contains("0912345678") && body.contains("https://agri.vn/provider/jobs"));
        assert!(!body.contains('{'), "every placeholder filled: {body}");
    }

    fn notice(note: Option<&str>) -> InquiryNotice {
        InquiryNotice {
            business_name: "Vườn Út Hiền".into(),
            requester_name: "Nguyễn Văn A".into(),
            requester_phone: "0912345678".into(),
            topic: "Múc bùn ao".into(),
            note: note.map(str::to_owned),
            inbox_path: "/provider/inquiries".into(),
        }
    }

    #[test]
    fn the_inquiry_email_has_who_to_call_and_the_inbox_link() {
        let body = inquiry_body(&texts(), "https://agri.vn/", &notice(None));
        assert!(body.contains("Nguyễn Văn A") && body.contains("0912345678"));
        assert!(body.contains("Needs: Múc bùn ao"));
        assert!(body.contains("https://agri.vn/provider/inquiries"));
        assert!(!body.contains("Note:"), "no empty note line");
        assert!(!body.contains('{'), "every placeholder filled: {body}");

        let noted = inquiry_body(&texts(), "https://agri.vn", &notice(Some("2 công")));
        assert!(noted.contains("Note: 2 công\n"));
    }

    #[test]
    fn reset_url_does_not_double_the_slash() {
        assert_eq!(
            reset_url("https://a.vn/", "t"),
            "https://a.vn/reset-password?token=t"
        );
        assert_eq!(
            reset_url("https://a.vn", "t"),
            "https://a.vn/reset-password?token=t"
        );
    }
}
