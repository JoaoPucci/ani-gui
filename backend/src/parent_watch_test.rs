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
    assert!(reported_within(until_eof(reader), Duration::from_secs(5)).await);
}

#[tokio::test]
async fn bytes_on_a_live_pipe_do_not_end_the_watch() {
    let (reader, mut writer) = std::io::pipe().expect("pipe");
    let mut watch = std::pin::pin!(until_eof(reader));
    writer.write_all(b"anything\n").expect("write");
    assert!(!reported_within(&mut watch, Duration::from_millis(300)).await);
    drop(writer);
    assert!(reported_within(&mut watch, Duration::from_secs(5)).await);
}
