use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{oneshot, Notify};

/// A server on a loopback port whose `/held` route answers only once
/// `release` is notified, and tells `entered` when a request reaches it.
struct Served {
    base: String,
    stop: oneshot::Sender<()>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
    done: tokio::task::JoinHandle<std::io::Result<()>>,
}

async fn serve(grace: Duration) -> Served {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let (e, r) = (entered.clone(), release.clone());
    let router = axum::Router::new().route(
        "/held",
        axum::routing::get(move || {
            let (e, r) = (e.clone(), r.clone());
            async move {
                e.notify_one();
                r.notified().await;
                "released"
            }
        }),
    );
    let (stop, stopped) = oneshot::channel::<()>();
    let done = tokio::spawn(serve_until(
        listener,
        router,
        async move {
            let _ = stopped.await;
        },
        grace,
    ));
    Served {
        base,
        stop,
        entered,
        release,
        done,
    }
}

#[tokio::test]
async fn a_server_with_nothing_in_flight_stops_at_once() {
    // The grace is far longer than the test waits: stopping an idle
    // server must not spend it.
    let served = serve(Duration::from_secs(60)).await;
    served.stop.send(()).expect("stop");
    let result = tokio::time::timeout(Duration::from_secs(5), served.done)
        .await
        .expect("an idle server stops without waiting out the grace")
        .expect("join");
    assert!(result.is_ok());
}

#[tokio::test]
async fn a_request_in_flight_gets_its_answer_before_the_server_stops() {
    let served = serve(Duration::from_secs(60)).await;
    let url = format!("{}/held", served.base);
    let request = tokio::spawn(async move { reqwest::get(url).await?.text().await });
    served.entered.notified().await;

    served.stop.send(()).expect("stop");
    // Still held: the stop has been asked for and the request has not
    // been answered, so the server is waiting on it.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!served.done.is_finished());

    served.release.notify_one();
    let body = request.await.expect("join").expect("answered");
    assert_eq!(body, "released");
    tokio::time::timeout(Duration::from_secs(5), served.done)
        .await
        .expect("the server stops once the request is answered")
        .expect("join")
        .expect("served");
}

#[tokio::test]
async fn a_request_whose_client_has_hung_up_does_not_hold_the_stop() {
    use tokio::io::AsyncWriteExt;
    // What a quit and a dead parent both look like from here: the
    // renderer is gone, so its connections are closed. The grace is
    // far longer than the test waits.
    let served = serve(Duration::from_secs(60)).await;
    let authority = served.base.strip_prefix("http://").expect("origin");
    let mut client = tokio::net::TcpStream::connect(authority)
        .await
        .expect("connect");
    client
        .write_all(format!("GET /held HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
        .await
        .expect("request");
    served.entered.notified().await;
    drop(client);

    served.stop.send(()).expect("stop");
    tokio::time::timeout(Duration::from_secs(5), served.done)
        .await
        .expect("a request nobody is waiting for is dropped, not waited out")
        .expect("join")
        .expect("served");
}

#[tokio::test]
async fn a_request_that_never_finishes_holds_the_stop_for_the_grace_and_no_longer() {
    let grace = Duration::from_millis(400);
    let served = serve(grace).await;
    let url = format!("{}/held", served.base);
    // Never released, and the client never hangs up.
    let request = tokio::spawn(async move { reqwest::get(url).await });
    served.entered.notified().await;

    let asked = Instant::now();
    served.stop.send(()).expect("stop");
    tokio::time::timeout(Duration::from_secs(5), served.done)
        .await
        .expect("the grace bounds the wait")
        .expect("join")
        .expect("served");
    assert!(
        asked.elapsed() >= grace,
        "the request got its grace: {:?}",
        asked.elapsed()
    );
    request.abort();
}

/// Sets its flag when dropped — the shape of the guard that kills a
/// download tool's process group when its task is torn down.
struct Dropped(Arc<AtomicBool>);

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn teardown_drops_running_tasks_and_does_not_wait_out_a_stuck_blocking_call() {
    // Stands in for a blocking call that will not return in any time
    // a stop can wait: a read on a dead mount, a lock held elsewhere.
    const STUCK: Duration = Duration::from_secs(6);
    let limit = Duration::from_millis(300);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = Dropped(dropped.clone());
    runtime.spawn(async move {
        let _guard = guard;
        std::future::pending::<()>().await;
    });
    let (running_tx, running_rx) = std::sync::mpsc::channel();
    runtime.spawn_blocking(move || {
        let _ = running_tx.send(());
        std::thread::sleep(STUCK);
    });
    running_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the blocking call is running");

    let asked = Instant::now();
    teardown(runtime, limit);
    let took = asked.elapsed();

    assert!(
        took < STUCK / 2,
        "the limit bounds the teardown, not the stuck call: {took:?}"
    );
    assert!(
        dropped.load(Ordering::SeqCst),
        "a task still running is dropped, so its guard runs"
    );
}
