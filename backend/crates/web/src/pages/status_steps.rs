//! The next steps of a contact request or a job post: the buttons a list
//! row or an inbox card offers (`molecules/row_action.html` `step`), never a
//! select of every status. Each status offers only the moves that make
//! sense from it; "spam" is a link to the row's confirmation (it takes the
//! record out of the leads and out of the providers' sight), the others one
//! click. The use cases still take any status of their list: this is what
//! the pages offer, not a rule of the records.

use domain::constants::{
    INQUIRY_STATUS_CLOSED, INQUIRY_STATUS_CONTACTED, INQUIRY_STATUS_NEW, INQUIRY_STATUS_SPAM,
    JOB_STATUS_CLOSED, JOB_STATUS_OPEN, JOB_STATUS_SPAM,
};
use serde::Serialize;

/// One button: the status it sets and how it looks (`look`: `contacted`,
/// `done`, `reopen`, `spam`; the macro words and colours it). `primary`
/// marks the one move a record still being handled waits for: a filled
/// button, so a row reads at a glance; the others stay outlined, and a
/// finished record (only "reopen") has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Step {
    pub to: &'static str,
    pub look: &'static str,
    pub primary: bool,
}

const fn step(to: &'static str, look: &'static str) -> Step {
    Step {
        to,
        look,
        primary: false,
    }
}

const fn main_step(to: &'static str, look: &'static str) -> Step {
    Step {
        to,
        look,
        primary: true,
    }
}

/// The steps from an inquiry's status: a new one is called back, done or
/// spam; a contacted one done or spam; a done one reopens as contacted (the
/// farmer may still review), a spam one as new.
pub fn inquiry_steps(status: &str) -> &'static [Step] {
    const FROM_NEW: &[Step] = &[
        main_step(INQUIRY_STATUS_CONTACTED, "contacted"),
        step(INQUIRY_STATUS_CLOSED, "done"),
        step(INQUIRY_STATUS_SPAM, "spam"),
    ];
    const FROM_CONTACTED: &[Step] = &[
        main_step(INQUIRY_STATUS_CLOSED, "done"),
        step(INQUIRY_STATUS_SPAM, "spam"),
    ];
    const FROM_CLOSED: &[Step] = &[step(INQUIRY_STATUS_CONTACTED, "reopen")];
    const FROM_SPAM: &[Step] = &[step(INQUIRY_STATUS_NEW, "reopen")];
    match status {
        INQUIRY_STATUS_NEW => FROM_NEW,
        INQUIRY_STATUS_CONTACTED => FROM_CONTACTED,
        INQUIRY_STATUS_CLOSED => FROM_CLOSED,
        _ => FROM_SPAM,
    }
}

/// The steps from a job post's status: an open one is done or spam, the
/// others reopen.
pub fn job_steps(status: &str) -> &'static [Step] {
    const FROM_OPEN: &[Step] = &[
        main_step(JOB_STATUS_CLOSED, "done"),
        step(JOB_STATUS_SPAM, "spam"),
    ];
    const REOPEN: &[Step] = &[step(JOB_STATUS_OPEN, "reopen")];
    if status == JOB_STATUS_OPEN {
        FROM_OPEN
    } else {
        REOPEN
    }
}

/// The locale key of an inquiry status's word (`atoms/inquiry_status.html`
/// words it in templates).
pub fn inquiry_status_key(status: &str) -> &'static str {
    match status {
        INQUIRY_STATUS_NEW => "inquiry_status.new",
        INQUIRY_STATUS_CONTACTED => "inquiry_status.contacted",
        INQUIRY_STATUS_CLOSED => "inquiry_status.closed",
        _ => "inquiry_status.spam",
    }
}

/// The locale key of a job post status's word (`atoms/job_status.html`).
pub fn job_status_key(status: &str) -> &'static str {
    match status {
        JOB_STATUS_OPEN => "job_status.open",
        JOB_STATUS_CLOSED => "job_status.closed",
        _ => "job_status.spam",
    }
}

/// Whether `steps` offer marking spam: the row's confirmation may open.
pub fn offers_spam(steps: &[Step]) -> bool {
    steps.iter().any(|s| s.look == "spam")
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::constants::{INQUIRY_STATUSES, JOB_STATUSES};

    #[test]
    fn every_status_offers_a_way_out_and_never_itself() {
        for status in INQUIRY_STATUSES {
            let steps = inquiry_steps(status);
            assert!(!steps.is_empty(), "{status}");
            assert!(steps.iter().all(|s| s.to != *status), "{status}");
            assert!(steps.iter().all(|s| INQUIRY_STATUSES.contains(&s.to)));
        }
        for status in JOB_STATUSES {
            let steps = job_steps(status);
            assert!(!steps.is_empty(), "{status}");
            assert!(steps.iter().all(|s| s.to != *status), "{status}");
            assert!(steps.iter().all(|s| JOB_STATUSES.contains(&s.to)));
        }
    }

    #[test]
    fn a_record_being_handled_has_one_main_step_and_a_finished_one_none() {
        let mains = |steps: &[Step]| steps.iter().filter(|s| s.primary).count();
        assert_eq!(mains(inquiry_steps(INQUIRY_STATUS_NEW)), 1);
        assert_eq!(mains(inquiry_steps(INQUIRY_STATUS_CONTACTED)), 1);
        assert_eq!(mains(inquiry_steps(INQUIRY_STATUS_CLOSED)), 0);
        assert_eq!(mains(inquiry_steps(INQUIRY_STATUS_SPAM)), 0);
        assert_eq!(mains(job_steps(JOB_STATUS_OPEN)), 1);
        assert_eq!(mains(job_steps(JOB_STATUS_CLOSED)), 0);
        assert_eq!(mains(job_steps(JOB_STATUS_SPAM)), 0);
        for status in INQUIRY_STATUSES {
            assert!(inquiry_steps(status)
                .iter()
                .all(|s| !(s.primary && s.look == "spam")));
        }
    }

    #[test]
    fn spam_is_offered_only_to_what_is_still_being_handled() {
        assert!(offers_spam(inquiry_steps(INQUIRY_STATUS_NEW)));
        assert!(offers_spam(inquiry_steps(INQUIRY_STATUS_CONTACTED)));
        assert!(!offers_spam(inquiry_steps(INQUIRY_STATUS_CLOSED)));
        assert!(!offers_spam(inquiry_steps(INQUIRY_STATUS_SPAM)));
        assert!(offers_spam(job_steps(JOB_STATUS_OPEN)));
        assert!(!offers_spam(job_steps(JOB_STATUS_SPAM)));
    }
}
