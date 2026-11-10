//! The Windows App Paths registration, read through `reg.exe`. Split
//! from [`super`] so each file stays inside the CRAP gate's per-file
//! bar.

/// The default value of a `reg query … /ve` answer: the text after its
/// `REG_SZ` or `REG_EXPAND_SZ` type, without surrounding quotes. The
/// value's name column is localized ("(Default)", "(Padrão)", …), so
/// only the type marks the line.
#[must_use]
pub fn parse_reg_default(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (_, value) = line
            .split_once("    REG_EXPAND_SZ    ")
            .or_else(|| line.split_once("    REG_SZ    "))?;
        let value = value.trim().trim_matches('"').trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// How long one `reg.exe` query may take before it is given up and
/// killed. It answers in milliseconds; a hung one must not hold
/// Settings or a launch.
#[cfg_attr(not(windows), allow(dead_code))] // used by the Windows query
const REG_LIMIT: std::time::Duration = std::time::Duration::from_secs(2);

/// The App Paths registration of `file` (e.g. `vlc.exe`): per-user,
/// then machine-wide in the 64-bit and the 32-bit view. Asked through
/// `reg.exe` ([`reg_command`], [`reg_output`]), without a console
/// window, each query bounded by [`REG_LIMIT`].
#[cfg(windows)]
pub(super) fn registered_app_path(file: &str) -> Option<String> {
    let key = format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{file}");
    let reg = std::env::var("SystemRoot")
        .map(|root| format!("{root}\\System32\\reg.exe"))
        .unwrap_or_else(|_| "reg.exe".to_string());
    let queries: [(&str, Option<&str>); 3] = [
        ("HKCU", None),
        ("HKLM", Some("/reg:64")),
        ("HKLM", Some("/reg:32")),
    ];
    queries.into_iter().find_map(|(hive, view)| {
        let mut args = vec![
            "query".to_string(),
            format!("{hive}\\{key}"),
            "/ve".to_string(),
        ];
        args.extend(view.map(str::to_string));
        let out = reg_output(reg_command(&reg, &args), REG_LIMIT)?;
        out.status
            .success()
            .then(|| parse_reg_default(&String::from_utf8_lossy(&out.stdout)))
            .flatten()
    })
}

/// The `reg.exe` command for one query (AGENTS.md §5): a tokio
/// command, killed if dropped, with `TERM=dumb` and `NO_COLOR=1`, no
/// stdin, and on Windows no console window. Its output is collected by
/// `output()`, which captures stdout and stderr; only stdout is read.
#[cfg_attr(not(windows), allow(dead_code))] // used by the Windows query and the tests
pub(super) fn reg_command(program: &str, args: &[String]) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// The output of `cmd`, or none when it can't start or runs past
/// `limit`, in which case it is dropped and so killed. Runs on a thread
/// of its own with its own small runtime, so it can be called from any
/// context, a blocking-pool thread or inside an async task alike.
#[cfg_attr(not(windows), allow(dead_code))] // used by the Windows query and the tests
pub(super) fn reg_output(
    mut cmd: tokio::process::Command,
    limit: std::time::Duration,
) -> Option<std::process::Output> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        runtime.block_on(async move { tokio::time::timeout(limit, cmd.output()).await.ok()?.ok() })
    })
    .join()
    .ok()
    .flatten()
}

/// No registry off Windows.
#[cfg(not(windows))]
pub(super) fn registered_app_path(_file: &str) -> Option<String> {
    None
}

/// `%NAME%` replaced by the variable's value; an unknown one is kept.
pub(super) fn expand(text: &str, var: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(len) = after.find('%') else { break };
        out.push_str(&rest[..start]);
        let key = &after[..len];
        match var(key) {
            Some(v) if !key.is_empty() => out.push_str(&v),
            _ => {
                out.push('%');
                out.push_str(key);
                out.push('%');
            }
        }
        rest = &after[len + 1..];
    }
    out.push_str(rest);
    out
}
