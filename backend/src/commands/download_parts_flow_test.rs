//! A whole transfer against the working folder: what the user's
//! download folder holds while a download runs and after it ends.
//!
//! Unix-only, like every case that stages a stub tool as a shell
//! script; the folder's own behaviour on Windows — hidden, swept,
//! removed — is covered by `download_parts_test.rs`, which stages none.

use super::parts::{parts_dir, PARTS_DIR};
use super::spawn_download_tool;
use crate::scraper::provider::StreamSource;

fn stage_tool(dir: &std::path::Path, name: &str, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{script}\n")).expect("write stub");
    let mut perms = std::fs::metadata(&p).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&p, perms).expect("chmod");
}

fn source() -> StreamSource {
    StreamSource {
        master_url: "https://cdn.example/x/master.m3u8".into(),
        referer: None,
        subtitles: Vec::new(),
    }
}

/// The names in `dir` a person browsing it would see; the lock
/// directory is its own change's to tidy.
fn visible(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != ".ani-gui-locks")
        .collect();
    names.sort();
    names
}

/// A yt-dlp stub that writes what the real one does beside its `-o`
/// path — the `.part`, a fragment in flight and its own partial, the
/// `.ytdl` resume state — records the `-o` path, and then either waits
/// for `release` to appear or not at all, finishing with `exit`.
fn ytdlp_writing_its_temporaries(
    seen: &std::path::Path,
    release: Option<&std::path::Path>,
    exit: i32,
) -> String {
    let wait = release.map_or(String::new(), |r| {
        format!(
            "while [ ! -e {r} ]; do sleep 0.02; done\n",
            r = crate::stub_file::sh_quote(r)
        )
    });
    format!(
        "out=\"\"\nprev=\"\"\nfor a in \"$@\"; do if [ \"$prev\" = \"-o\" ]; then out=\"$a\"; fi; prev=\"$a\"; done\n\
         printf '%s' \"$out\" > {seen}\n\
         printf x > \"$out.part\"\nprintf x > \"$out.part-Frag12\"\nprintf x > \"$out.part-Frag16.part\"\nprintf x > \"$out.ytdl\"\n\
         {wait}\
         if [ {exit} -eq 0 ]; then mv \"$out.part\" \"$out\"; rm -f \"$out.part-Frag12\" \"$out.part-Frag16.part\" \"$out.ytdl\"; fi\n\
         exit {exit}",
        seen = crate::stub_file::sh_quote(seen),
    )
}

async fn until_exists(path: &std::path::Path) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while !path.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn while_a_download_runs_its_temporaries_are_in_the_hidden_working_folder() {
    // On Windows a leading dot hides nothing, so every name the
    // transfer writes beside the target was a file in the user's
    // Downloads folder until the episode finished.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let seen = bin.path().join("seen");
    let release = bin.path().join("release");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &ytdlp_writing_its_temporaries(&seen, Some(&release), 0),
    );
    let path_env = bin.path().display().to_string();
    let dir = dest.path().to_path_buf();
    let running = tokio::spawn(async move {
        spawn_download_tool(
            &source(),
            &dir,
            "Hidden Show Episode 1",
            None,
            &path_env,
            std::time::Duration::from_secs(30),
            &mut |_l: &str| {},
        )
        .await
    });
    until_exists(&seen).await;
    let out = std::path::PathBuf::from(std::fs::read_to_string(&seen).expect("the -o path"));
    assert_eq!(
        out.parent(),
        Some(parts_dir(dest.path()).as_path()),
        "the tool writes inside the working folder"
    );
    until_exists(&std::path::PathBuf::from(format!("{}.ytdl", out.display()))).await;
    assert_eq!(
        visible(dest.path()),
        vec![PARTS_DIR.to_string()],
        "mid-transfer, the download folder shows nothing but the working folder"
    );

    std::fs::write(&release, b"").expect("release");
    running
        .await
        .expect("joined")
        .expect("the download succeeds");
    assert_eq!(
        visible(dest.path()),
        vec!["Hidden Show Episode 1.mp4".to_string()],
        "only the finished file is left, and the working folder is gone"
    );
}

#[tokio::test]
async fn a_failed_download_takes_its_working_folder_with_it() {
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let seen = bin.path().join("seen");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &ytdlp_writing_its_temporaries(&seen, None, 1),
    );
    let got = spawn_download_tool(
        &source(),
        dest.path(),
        "Failed Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |_l: &str| {},
    )
    .await;
    assert!(got.is_err(), "the tool failed and no fallback exists");
    assert!(seen.exists(), "the tool ran");
    assert!(
        visible(dest.path()).is_empty(),
        "nothing is left: {:?}",
        visible(dest.path())
    );
}

#[tokio::test]
async fn a_download_sweeps_what_a_killed_run_left_and_keeps_a_running_ones() {
    // Windows ends the backend without unwinding on every quit, so a
    // download in flight leaves its files and its unlocked hold. The
    // next download into the same folder removes them — and leaves a
    // download that is still running alone.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let dir = parts_dir(dest.path());
    std::fs::create_dir(&dir).expect("working folder");
    let dead = ".ani-gui-4242-0123456789abcdef";
    std::fs::write(dir.join(format!("{dead}.hold")), b"").expect("dead hold");
    let dead_part = dir.join(format!("{dead}-77.part.mp4.part-Frag12"));
    std::fs::write(&dead_part, b"most of an episode").expect("dead part");
    let running = super::parts::Hold::take(dest.path(), false).expect("running");
    let running_part = running.part(".part.mp4.part");
    std::fs::write(&running_part, b"in flight").expect("running part");

    let seen = bin.path().join("seen");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &ytdlp_writing_its_temporaries(&seen, None, 0),
    );
    spawn_download_tool(
        &source(),
        dest.path(),
        "Next Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |_l: &str| {},
    )
    .await
    .expect("the download succeeds");

    assert!(!dead_part.exists(), "a killed run's fragment is swept");
    assert!(!dir.join(format!("{dead}.hold")).exists(), "with its hold");
    assert!(running_part.exists(), "a running download's part is kept");
    assert_eq!(
        visible(dest.path()),
        vec![PARTS_DIR.to_string(), "Next Show Episode 1.mp4".to_string()],
        "the running download keeps the working folder"
    );
    drop(running);
    assert_eq!(
        visible(dest.path()),
        vec!["Next Show Episode 1.mp4".to_string()]
    );
}

#[tokio::test]
async fn a_subtitles_scratch_is_in_the_working_folder_too() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Show Episode 3.en.vtt");
    let claim = super::claim_new(&target).await.expect("claimed");
    assert_eq!(
        visible(dest.path()),
        vec![PARTS_DIR.to_string()],
        "the unfinished track is not a name in the download folder"
    );
    drop(claim);
    assert!(
        visible(dest.path()).is_empty(),
        "and a dropped claim takes the folder with it"
    );
}
