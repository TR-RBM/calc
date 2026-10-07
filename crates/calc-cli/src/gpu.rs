use std::sync::{Arc, Mutex};

use calc_app::{LazyGpuBackend, Session, gpu_refusal_message, lazy_gpu_backend};
use calc_i18n::{Locale, Localized, render};

use crate::output::Output;

pub(crate) type ProbeOutcome = Arc<Mutex<Option<Arc<LazyGpuBackend>>>>;

pub(crate) fn probing_session(session: Session) -> (Session, ProbeOutcome) {
    let outcome: ProbeOutcome = Arc::new(Mutex::new(None));
    let asked = Arc::clone(&outcome);
    let session = session.with_further_backends(move || {
        let (backend, proxy) = lazy_gpu_backend();
        if let Ok(mut held) = asked.lock() {
            *held = Some(proxy);
        }
        vec![backend]
    });
    (session, outcome)
}

pub(crate) fn refusal_row(outcome: &ProbeOutcome, locale: &Locale) -> Option<Localized> {
    let proxy = outcome.lock().ok()?.clone()?;
    let refusal = proxy.refusal()?;
    Some(render(&gpu_refusal_message(&refusal), locale))
}

#[derive(Default)]
pub(crate) struct Probes {
    outcomes: Vec<ProbeOutcome>,
}

impl Probes {
    pub(crate) fn watch(&mut self, outcome: ProbeOutcome) {
        self.outcomes.push(outcome);
    }

    pub(crate) fn report(&self, locale: &Locale, errors: &mut Output<'_>) {
        for outcome in &self.outcomes {
            if let Some(row) = refusal_row(outcome, locale) {
                let _ = errors.line(&row);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_app::{FixedClock, UtcTimestamp, registered_backends};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn session() -> Session {
        let clock = FixedClock::new(UtcTimestamp::from_milliseconds_since_unix_epoch(0), 1);
        Session::new(Box::new(clock), registered_backends())
    }

    fn counting_session(calls: &Arc<AtomicUsize>) -> Session {
        let counted = Arc::clone(calls);
        session().with_further_backends(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        })
    }

    #[test]
    fn an_exact_line_never_asks_for_a_further_backend() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut session = counting_session(&calls);

        session.enter("2 + 3").expect("line is entered");

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_line_that_needs_machine_arithmetic_asks_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut session = counting_session(&calls);

        session.enter("to_f64(1/3)").expect("line is entered");
        session.enter("to_f64(1/7)").expect("line is entered");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn no_refusal_is_reported_before_the_probe_has_run() {
        let (mut session, outcome) = probing_session(session());

        session.enter("2 + 3").expect("line is entered");

        assert!(refusal_row(&outcome, &Locale::source()).is_none());
    }
}
