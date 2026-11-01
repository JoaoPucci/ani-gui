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
//!
//! The other way round — the test holds the lock and asserts that what
//! waits on it has not gone ahead — needs to know the waiter got as far
//! as the lock. [`queue`] watches a lock the app notes its waits on
//! ([`queued`]), and [`Queue::joined`] returns once one has queued. The
//! same note marks a retry of an executable found busy and a wait on a
//! claimed name, for a test that holds the file. [`shared`] sees a
//! waiter at a lock handed out as an `Arc`, and [`until`] polls a state
//! the subject reaches that the test can read but not await.
//!
//! The async waits' deadlines (`until`, `shared`, `Queue::joined`) are
//! on tokio's clock, and on a paused clock that clock jumps ahead
//! whenever the runtime is idle — while a subject works on a thread of
//! its own, say — so those are for tests on the real clock.
//!
//! A test that times a subject's deadline once its request is held
//! runs it through [`frozen_until`], whose clock stands still until the
//! hold is reached, or [`paused_once`], whose clock runs until then: a
//! loaded machine can spend the deadline on the way to the hold, so the
//! second is for a subject that sleeps on its way there.

use std::collections::BTreeMap;
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

/// Returns once `reached` holds: a state the subject reaches that the
/// test can read but not await, polled.
///
/// # Panics
/// When it has not within [`QUEUE_DEADLINE`].
pub(crate) async fn until(what: &str, reached: impl Fn() -> bool) {
    let deadline = tokio::time::Instant::now() + QUEUE_DEADLINE;
    while !reached() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "never reached: {what}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
}

/// Returns once `lock` has more than `handles` handles on it.
///
/// A lock handed out as an `Arc` from a map — a row's, a show's — is
/// cloned by whoever waits for it and kept for the wait, so a handle
/// over the ones the map and the test keep is a waiter at the lock, or
/// its holder. For a test that holds the lock, that is a waiter behind
/// it, with no step left before the wait that could let it through.
///
/// # Panics
/// When no waiter has come within [`QUEUE_DEADLINE`].
pub(crate) async fn shared<T>(lock: &Arc<T>, handles: usize) {
    let deadline = tokio::time::Instant::now() + QUEUE_DEADLINE;
    while Arc::strong_count(lock) <= handles {
        assert!(
            tokio::time::Instant::now() < deadline,
            "nothing came to the lock"
        );
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
}

/// Waits that found their lock taken, counted by what the lock is
/// known by ([`queued`]).
///
/// A test that holds a lock and asserts that what waits on it has not
/// gone ahead has to know the waiter got as far as the lock. Without
/// that, a slow subject that has not reached the lock yet satisfies the
/// assertion as well as one queued on it, and the test passes without
/// testing anything. A lock the app holds for tests to watch notes each
/// wait that finds it taken, just before the wait begins, and the test
/// awaits that note ([`Queue::joined`]) before it asserts.
static QUEUED: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());

/// Signalled whenever [`QUEUED`] changes.
static QUEUED_CHANGED: Condvar = Condvar::new();

/// How long a test waits for a waiter to queue before it fails. Long
/// enough for any machine to get there; a waiter that never comes ends
/// the test rather than hanging the suite.
const QUEUE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

/// Called by a lock's waiter that found the lock taken, just before it
/// waits: nothing between this and the wait lets it go ahead. `on` is
/// what the lock is known by — the path it guards, or [`lock_key`] for
/// a lock in memory. A retry of a busy executable and a wait on a
/// claimed name note the same way, under the file's path.
pub(crate) fn queued(on: &str) {
    let mut queued = QUEUED.lock().unwrap_or_else(PoisonError::into_inner);
    *queued.entry(on.to_owned()).or_default() += 1;
    QUEUED_CHANGED.notify_all();
}

/// What a lock in memory is known by in [`queued`]: where it is.
pub(crate) fn lock_key<T: ?Sized>(lock: &T) -> String {
    format!("{lock:p}")
}

/// A test's watch on one lock's queue, opened before the subject
/// starts. Counts only the waits that come after it, so a lock at an
/// address an earlier test's lock had is not taken for queued by that
/// test's waits.
pub(crate) struct Queue {
    on: String,
    before: usize,
}

/// Watch the queue of the lock known by `on`.
pub(crate) fn queue(on: impl Into<String>) -> Queue {
    let on = on.into();
    let before = queued_on(&QUEUED.lock().unwrap_or_else(PoisonError::into_inner), &on);
    Queue { on, before }
}

fn queued_on(queued: &BTreeMap<String, usize>, on: &str) -> usize {
    queued.get(on).copied().unwrap_or(0)
}

impl Queue {
    /// The same watch, counting only the waits that come after now.
    #[cfg(unix)]
    pub(crate) fn again(self) -> Self {
        queue(self.on)
    }

    /// Whether a waiter has queued on the lock since the watch opened.
    pub(crate) fn has_joined(&self) -> bool {
        queued_on(
            &QUEUED.lock().unwrap_or_else(PoisonError::into_inner),
            &self.on,
        ) > self.before
    }

    /// Returns once a waiter has queued on the lock since the watch
    /// opened.
    ///
    /// # Panics
    /// When none has within [`QUEUE_DEADLINE`].
    pub(crate) async fn joined(&self) {
        let deadline = tokio::time::Instant::now() + QUEUE_DEADLINE;
        while !self.has_joined() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "nothing queued on {}",
                self.on
            );
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    }

    /// [`Self::joined`], for a test on a thread of its own, watching
    /// `ahead` meanwhile: whether the waiter has gone past the lock,
    /// which it can only have done without queueing on it.
    ///
    /// # Panics
    /// When the waiter went ahead, or none has queued within
    /// [`QUEUE_DEADLINE`].
    pub(crate) fn joined_blocking(&self, ahead: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + QUEUE_DEADLINE;
        let mut queued = QUEUED.lock().unwrap_or_else(PoisonError::into_inner);
        while queued_on(&queued, &self.on) <= self.before {
            assert!(!ahead(), "went ahead without queueing on {}", self.on);
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(!left.is_zero(), "nothing queued on {}", self.on);
            queued = QUEUED_CHANGED
                .wait_timeout(queued, left.min(std::time::Duration::from_millis(5)))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// A bounded wait timed on tokio's clock, from the call
/// ([`paused_once`], [`frozen_until`]): how long it took, the latest it
/// can have set its deadline, and when it reached its hold.
pub(crate) struct Timed<T> {
    pub(crate) out: T,
    took: std::time::Duration,
    /// [`paused_once`]: the pause, since the wait set its deadline
    /// somewhere between the call and the pause. [`frozen_until`]:
    /// nothing, the clock having stood still from the call.
    set_by: std::time::Duration,
    // Read only by the Unix-only fallback-deadline case.
    #[cfg_attr(not(unix), allow(dead_code))]
    reached_at: std::time::Duration,
    #[cfg_attr(not(unix), allow(dead_code))]
    started: tokio::time::Instant,
}

impl<T> Timed<T> {
    /// The wait's output, once it is seen to have returned by
    /// `deadline` after the latest moment it can have set it. From then
    /// on only timers move the clock, so a wait still out returns at the
    /// first deadline that ends it, however slow the machine.
    pub(crate) fn ended_by(self, deadline: std::time::Duration, what: &str) -> T {
        // Tokio's timers fire on millisecond ticks, rounded up from the
        // instant they were set: up to two of them past the deadline.
        let tick = std::time::Duration::from_millis(2);
        let bound = self.set_by + deadline + tick;
        assert!(
            self.took <= bound,
            "{what}: took {:?} on tokio's clock, past the {deadline:?} deadline \
             (set by {:?} at the latest)",
            self.took,
            self.set_by
        );
        self.out
    }
}

impl<T> Timed<T> {
    /// When the wait reached its hold, from the call, and the call's
    /// moment on tokio's clock. Read only by the fallback-deadline case,
    /// which stages shell stubs and so runs on Unix alone; gated with it
    /// so Windows builds carry no dead code.
    #[cfg(unix)]
    pub(crate) fn reached_at(&self) -> std::time::Duration {
        self.reached_at
    }

    /// See [`Self::reached_at`].
    #[cfg(unix)]
    pub(crate) fn started(&self) -> tokio::time::Instant {
        self.started
    }
}

/// Run `subject` on the real clock until `reached` resolves — the
/// requests it makes have reached the holds that keep them — then on a
/// paused clock to its end, and resume the clock. Paused, tokio moves
/// time on only to the next timer, so the subject's own deadline is
/// what ends it, at that deadline exactly, and a deadline that grew is
/// seen to have. The test must run on a current-thread runtime.
///
/// Paused, the clock also jumps past any request still out on the real
/// network, so the subject's outcome says only that a deadline ended it.
/// A test that reads the outcome as well waits, in `reached`, for every
/// answer the subject still needs, or reads the outcome from a call on
/// the real clock.
pub(crate) async fn paused_once<F: std::future::Future>(
    subject: F,
    reached: impl std::future::Future<Output = ()>,
) -> Timed<F::Output> {
    let start = tokio::time::Instant::now();
    tokio::pin!(subject);
    tokio::select! {
        out = &mut subject => {
            let took = start.elapsed();
            Timed { out, took, set_by: took, reached_at: took, started: start }
        }
        () = reached => {
            let paused_at = start.elapsed();
            tokio::time::pause();
            let out = subject.await;
            let took = start.elapsed();
            tokio::time::resume();
            Timed { out, took, set_by: paused_at, reached_at: paused_at, started: start }
        }
    }
}

/// Run `subject` on a paused clock that stands still until `reached`
/// resolves — the requests it makes have reached the holds that keep
/// them — and from then on moves only to the next timer. No deadline
/// the subject sets can run out before it is held, so its deadline is
/// what ends it, at that deadline exactly, however long the machine
/// took to bring it to the hold. The test must run on a current-thread
/// runtime.
///
/// [`paused_once`] runs the way to the hold on the real clock, and a
/// machine slow enough there spends the deadline before the request
/// is out; this one does not. While the clock stands still, a timer the
/// subject waits on before the hold never fires, so a subject that
/// sleeps on its way there is for [`paused_once`].
///
/// A blocking task keeps the clock still: tokio does not move a paused
/// clock on for an idle current-thread runtime while one runs. So does
/// any other blocking task still running once the hold is reached, and
/// a hold the test keeps waits on the runtime ([`Hold::wait`]) or on a
/// thread of its own (a [`Held`] answer), not in a blocking task.
///
/// # Panics
/// When the subject ends before it reaches the hold. A subject that
/// has not reached it within [`QUEUE_DEADLINE`] of real time gets its
/// clock back — once no other blocking task runs, the test's own
/// watchers in `reached` among them — and its deadline then ends it
/// before the hold.
pub(crate) async fn frozen_until<F: std::future::Future>(
    subject: F,
    reached: impl std::future::Future<Output = ()>,
) -> Timed<F::Output> {
    tokio::time::pause();
    let start = tokio::time::Instant::now();
    let (hold_still, still) = std::sync::mpsc::channel::<()>();
    let standing = tokio::task::spawn_blocking(move || {
        let _ = still.recv_timeout(QUEUE_DEADLINE);
    });
    tokio::pin!(subject);
    let ended_first = tokio::select! {
        () = reached => false,
        _ = &mut subject => true,
    };
    drop(hold_still);
    if ended_first {
        tokio::time::resume();
        panic!("the subject ended before it reached the hold");
    }
    let reached_at = start.elapsed();
    standing.await.expect("the clock's hold ends");
    let out = subject.await;
    let took = start.elapsed();
    tokio::time::resume();
    Timed {
        out,
        took,
        set_by: std::time::Duration::ZERO,
        reached_at,
        started: start,
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

#[tokio::test]
async fn a_queue_sees_a_wait_noted_after_it_opened() {
    let on = "held-test-queue-sees";
    let queue = queue(on);
    let waiter = tokio::spawn(async move { queued(on) });
    queue.joined().await;
    waiter.await.expect("noted");
}

#[test]
fn a_queue_opened_after_a_wait_does_not_count_it() {
    let on = "held-test-queue-counts-after";
    queued(on);
    let queue = queue(on);
    assert!(!queue.has_joined(), "the wait before the watch is not seen");
    queued(on);
    assert!(queue.has_joined(), "the wait after it is");
    queue.joined_blocking(|| false);
}

#[test]
fn a_lock_in_memory_is_known_by_where_it_is() {
    let (a, b) = (Mutex::new(()), Mutex::new(()));
    assert_eq!(lock_key(&a), lock_key(&a));
    assert_ne!(lock_key(&a), lock_key(&b));
}

#[tokio::test]
async fn a_waiter_for_a_shared_lock_is_seen_at_it() {
    let lock = Arc::new(tokio::sync::Mutex::new(()));
    let held = lock.lock().await;
    let waiter = {
        let lock = Arc::clone(&lock);
        tokio::spawn(async move { drop(lock.lock().await) })
    };
    shared(&lock, 1).await;
    assert!(!waiter.is_finished(), "behind the lock");
    drop(held);
    waiter.await.expect("let through");
}

#[tokio::test]
async fn a_wait_timed_once_its_request_is_held_returns_at_its_deadline() {
    let (hold, gate) = gate();
    let deadline = std::time::Duration::from_millis(300);
    let held = tokio::spawn(async move { hold.wait().await });
    let timed = paused_once(
        tokio::time::timeout(deadline, async { std::future::pending::<()>().await }),
        gate.asked(),
    )
    .await;
    let out = timed.ended_by(deadline, "a wait ended by its deadline");
    assert!(out.is_err(), "the deadline ended it");
    gate.release();
    held.await.expect("released");
}

/// The way to the hold takes longer than the deadline, on the real
/// clock: a clock that ran meanwhile would end the wait before it was
/// held. Frozen until the hold, it ends at the deadline exactly.
#[tokio::test]
async fn a_wait_frozen_until_its_request_is_held_returns_at_its_deadline_however_slow_the_way() {
    let (hold, gate) = gate();
    let deadline = std::time::Duration::from_millis(300);
    // The slow way runs on a thread of its own, as a connection's other
    // end does: a blocking task would keep the clock standing by itself,
    // and the test would pass without the hold `frozen_until` keeps.
    let held = std::thread::spawn(move || {
        std::thread::sleep(2 * deadline);
        hold.wait_blocking();
    });
    let timed = frozen_until(
        tokio::time::timeout(deadline, async { std::future::pending::<()>().await }),
        gate.asked(),
    )
    .await;
    assert!(timed.took >= deadline, "ended at {:?}", timed.took);
    let out = timed.ended_by(deadline, "a wait frozen until its hold");
    assert!(out.is_err(), "the deadline ended it");
    gate.release();
    held.join().expect("released");
}

#[tokio::test]
#[should_panic(expected = "before it reached the hold")]
async fn a_subject_that_ends_before_its_hold_is_seen_to() {
    let (_hold, gate) = gate();
    let _ = frozen_until(std::future::ready(()), gate.asked()).await;
}

#[tokio::test]
#[should_panic(expected = "past the")]
async fn a_wait_whose_deadline_grew_is_seen_past_the_one_it_was_given() {
    let (hold, gate) = gate();
    let held = tokio::spawn(async move { hold.wait().await });
    let timed = paused_once(
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            std::future::pending::<()>().await;
        }),
        gate.asked(),
    )
    .await;
    gate.release();
    let _ = held.await;
    let _ = timed.ended_by(std::time::Duration::from_millis(300), "a grown deadline");
}
