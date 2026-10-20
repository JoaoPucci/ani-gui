//! A gate a test holds a request at: the request is taken and
//! signalled, and goes no further until the test lets it.
//!
//! A test that has to act while something is outstanding — a read
//! while a lookup is out, a removal while a resolve waits — holds that
//! thing here instead of delaying it for a while and hoping the action
//! lands inside the delay. A delay is a guess about how fast the
//! machine is: on a loaded runner, or with the history's writes
//! queueing on a slow disk, the action lands after it and the test
//! goes red although nothing is wrong. A held request is outstanding
//! for exactly as long as the test needs it to be.
//!
//! [`gate`] makes the two ends. The [`Hold`] goes where the request is
//! served — a [`Held`] wiremock answer, a provider stand-in — and the
//! [`Gate`] stays with the test, which awaits [`Gate::asked`], acts,
//! and calls [`Gate::release`]. Dropping the gate releases it too, so
//! a test that fails before it lets go leaves nothing waiting.

use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// Whether the gate is open, readable from both a blocking and an
/// async waiter.
struct Open {
    flag: Mutex<bool>,
    changed: Condvar,
    opened: tokio::sync::watch::Sender<bool>,
}

/// The test's end of a gate.
pub(crate) struct Gate {
    asked: Arc<tokio::sync::Notify>,
    open: Arc<Open>,
}

/// The served end of a gate: whatever waits on it is held until the
/// test releases the [`Gate`].
#[derive(Clone)]
pub(crate) struct Hold {
    asked: Arc<tokio::sync::Notify>,
    open: Arc<Open>,
}

/// A closed gate and the hold it keeps.
pub(crate) fn gate() -> (Hold, Gate) {
    let asked = Arc::new(tokio::sync::Notify::new());
    let open = Arc::new(Open {
        flag: Mutex::new(false),
        changed: Condvar::new(),
        opened: tokio::sync::watch::channel(false).0,
    });
    (
        Hold {
            asked: Arc::clone(&asked),
            open: Arc::clone(&open),
        },
        Gate { asked, open },
    )
}

impl Gate {
    /// Returns once a request has reached the hold. A request that came
    /// before this was awaited counts: the signal is kept for it.
    pub(crate) async fn asked(&self) {
        self.asked.notified().await;
    }

    /// Let every held request, and every one still to come, through.
    pub(crate) fn release(&self) {
        *self
            .open
            .flag
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
        self.open.changed.notify_all();
        self.open.opened.send_replace(true);
    }
}

impl Drop for Gate {
    fn drop(&mut self) {
        self.release();
    }
}

impl Hold {
    /// Signal the request, then block the calling thread until the gate
    /// is released. For a server that answers on a thread of its own.
    pub(crate) fn wait_blocking(&self) {
        self.asked.notify_one();
        let mut open = self
            .open
            .flag
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        while !*open {
            open = self
                .open
                .changed
                .wait(open)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Signal the request, then wait until the gate is released.
    pub(crate) async fn wait(&self) {
        self.asked.notify_one();
        let mut opened = self.open.opened.subscribe();
        // The sender lives in `open`, which this hold keeps alive.
        let _ = opened.wait_for(|open| *open).await;
    }
}

/// A wiremock answer given only once the gate is released.
///
/// The mock server answers on a thread of its own, and with its state
/// locked, so the hold blocks both: until the test releases the gate,
/// every other request to the same server waits behind the held one,
/// and so do the server's own reads of that state —
/// `received_requests`, and the check it runs when it is dropped. A
/// test releases the gate before it reads the server, and declares the
/// gate after the server, so that the gate goes first however the test
/// ends.
pub(crate) struct Held {
    hold: Hold,
    then: wiremock::ResponseTemplate,
}

impl Held {
    pub(crate) fn new(hold: Hold, then: wiremock::ResponseTemplate) -> Self {
        Self { hold, then }
    }
}

impl wiremock::Respond for Held {
    fn respond(&self, _req: &wiremock::Request) -> wiremock::ResponseTemplate {
        self.hold.wait_blocking();
        self.then.clone()
    }
}

#[tokio::test]
async fn a_held_answer_waits_for_its_release() {
    // The server first, so the gate is dropped before it: a failure
    // while the answer is held lets it go before the server's drop
    // waits on the state the held answer keeps locked.
    let server = wiremock::MockServer::start().await;
    let (hold, gate) = gate();
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(Held::new(hold, wiremock::ResponseTemplate::new(204)))
        .mount(&server)
        .await;
    let request = tokio::spawn(reqwest::get(server.uri()));
    gate.asked().await;
    assert!(!request.is_finished(), "held until released");
    gate.release();
    let status = request.await.expect("join").expect("answered").status();
    assert_eq!(status, reqwest::StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn an_async_hold_waits_for_its_release() {
    let (hold, gate) = gate();
    let waiting = tokio::spawn(async move { hold.wait().await });
    gate.asked().await;
    assert!(!waiting.is_finished(), "held until released");
    gate.release();
    waiting.await.expect("released");
}

#[tokio::test]
async fn a_dropped_gate_lets_its_hold_go() {
    let (hold, gate) = gate();
    let waiting = tokio::spawn(async move { hold.wait().await });
    gate.asked().await;
    drop(gate);
    waiting.await.expect("released");
}
