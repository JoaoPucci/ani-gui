use super::*;
use std::io::Write;
use std::time::Duration;

/// Whether the watch reported the parent gone within `wait`. A sender
/// that vanished without reporting is not a report.
async fn flipped_within(rx: &mut tokio::sync::watch::Receiver<bool>, wait: Duration) -> bool {
    matches!(
        tokio::time::timeout(wait, rx.wait_for(|gone| *gone)).await,
        Ok(Ok(_))
    )
}

#[tokio::test]
async fn the_writer_closing_flips_the_watch() {
    let (reader, writer) = std::io::pipe().expect("pipe");
    let mut rx = watch_for_eof(reader);
    drop(writer);
    assert!(flipped_within(&mut rx, Duration::from_secs(5)).await);
}

#[tokio::test]
async fn bytes_on_a_live_pipe_do_not_flip_the_watch() {
    let (reader, mut writer) = std::io::pipe().expect("pipe");
    let mut rx = watch_for_eof(reader);
    writer.write_all(b"anything\n").expect("write");
    assert!(!flipped_within(&mut rx, Duration::from_millis(300)).await);
    drop(writer);
    assert!(flipped_within(&mut rx, Duration::from_secs(5)).await);
}
