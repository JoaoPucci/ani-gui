use super::*;
use std::io::Write;
use std::time::Duration;

/// Whether `watch` resolved — reported the parent gone — within `wait`.
async fn reported_within(watch: impl std::future::Future<Output = ()>, wait: Duration) -> bool {
    tokio::time::timeout(wait, watch).await.is_ok()
}

#[tokio::test]
async fn the_writer_closing_ends_the_watch() {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(writer);
    let watch = arm(reader, || {}).serving();
    assert!(reported_within(watch, Duration::from_secs(5)).await);
}

#[tokio::test]
async fn bytes_on_a_live_pipe_do_not_end_the_watch() {
    let (reader, mut writer) = std::io::pipe().expect("pipe");
    let mut watch = std::pin::pin!(arm(reader, || {}).serving());
    writer.write_all(b"anything\n").expect("write");
    assert!(!reported_within(&mut watch, Duration::from_millis(300)).await);
    drop(writer);
    assert!(reported_within(&mut watch, Duration::from_secs(5)).await);
}

// The two ways the watch can fail to watch. Neither says anything
// about the parent, and the parent is, as far as anyone knows, alive
// and using this backend — so neither may stop it. A watch that is not
// running costs an orphan if the parent later dies; a watch that
// reports falsely costs a working app its backend.

#[tokio::test]
async fn a_watch_that_cannot_start_is_not_a_report() {
    let (reader, _writer) = std::io::pipe().expect("pipe");
    let watch = arm_with(
        reader,
        || {},
        |_job| Err(std::io::Error::other("no threads left")),
    )
    .serving();
    assert!(!reported_within(watch, Duration::from_millis(300)).await);
}

#[tokio::test]
async fn a_watch_that_ends_without_reporting_is_not_a_report() {
    let (reader, _writer) = std::io::pipe().expect("pipe");
    // The thread "ran" and is gone, and never got as far as its report.
    let watch = arm_with(
        reader,
        || {},
        |job| {
            drop(job);
            Ok(())
        },
    )
    .serving();
    assert!(!reported_within(watch, Duration::from_millis(300)).await);
}
