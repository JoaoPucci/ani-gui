// The .deb's maintainer scripts, run rather than read.
//
// Chromium on Linux uses its namespace sandbox when it can start one
// and the setuid helper otherwise. On Ubuntu 23.10+ AppArmor grants
// user namespaces per program, so without a profile for the installed
// binary the .deb always fell through to the setuid helper. The
// postinst now also installs an AppArmor profile granting `userns` to
// `/opt/ani-gui/ani-gui.bin` — with the rules Ubuntu's own profiles for
// Chrome, VS Code and Obsidian carry — and loads it where AppArmor is active; the
// postrm unloads and deletes it on removal. The setuid helper stays as
// the fallback for hosts with no user namespaces at all.
//
// electron-builder ships both scripts verbatim as the package's
// postinst and postrm (checked against a built .deb with
// `dpkg-deb -e`). Each test runs one under `sh` with `DPKG_ROOT`
// pointing at a scratch tree — dpkg's own variable for installing
// into another root, which every path in the scripts honours — and
// stubs for the commands that would need root or a kernel: `chown`,
// `aa-enabled` and `apparmor_parser`, each logging how it was called.

const nodeTest = require("node:test");
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const SCRIPTS = path.join(__dirname, "..", "build-resources");
const POSTINST = path.join(SCRIPTS, "deb-after-install.sh");
const POSTRM = path.join(SCRIPTS, "deb-after-remove.sh");
const PROFILE = "etc/apparmor.d/ani-gui";

// The scripts are Debian maintainer scripts, run here under /bin/sh;
// on any other platform there is nothing to run them on.
const test = (name, fn) => nodeTest(name, { skip: process.platform !== "linux" }, fn);

// Running a script that does not honour DPKG_ROOT would reach the
// real /opt and /usr/local/bin. Refuse before running it.
function assertRooted(script) {
  assert.match(fs.readFileSync(script, "utf8"), /\$DPKG_ROOT\//, `${script} must prefix its paths with $DPKG_ROOT`);
}

/**
 * A scratch root shaped like an installed package. `apparmor` is
 * "active" (aa-enabled answers yes), "inactive" (it answers no),
 * "absent" (no aa-enabled at all); `abi4` says whether the host's
 * AppArmor ships ABI 4.0, the first to know the `userns` rule;
 * `parser` is what the stub apparmor_parser exits with.
 */
function host({ apparmor = "active", abi4 = true, parser = 0 } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "deb-scripts-"));
  const tree = path.join(root, "root");
  const bin = path.join(root, "bin");
  const log = path.join(root, "calls");
  fs.mkdirSync(path.join(tree, "opt", "ani-gui"), { recursive: true });
  fs.writeFileSync(path.join(tree, "opt", "ani-gui", "chrome-sandbox"), "", { mode: 0o755 });
  fs.writeFileSync(path.join(tree, "opt", "ani-gui", "ani-gui"), "#!/bin/sh\n", { mode: 0o755 });
  fs.mkdirSync(path.join(tree, "etc", "apparmor.d", "abi"), { recursive: true });
  if (abi4) fs.writeFileSync(path.join(tree, "etc", "apparmor.d", "abi", "4.0"), "");
  fs.mkdirSync(bin);
  // The scripts see only this directory on PATH, so a command the host
  // happens to have (its own aa-enabled, say) cannot stand in for a
  // stub; the few real tools they use are linked in by name.
  for (const tool of ["cat", "chmod", "mkdir", "ln", "readlink", "rm"]) {
    const real = ["/usr/bin", "/bin"].map((d) => path.join(d, tool)).find((p) => fs.existsSync(p));
    fs.symlinkSync(real, path.join(bin, tool));
  }
  const stub = (name, body) =>
    fs.writeFileSync(path.join(bin, name), `#!/bin/sh\necho "${name} $*" >> "${log}"\n${body}\n`, { mode: 0o755 });
  stub("chown", "exit 0");
  stub("apparmor_parser", `exit ${parser}`);
  if (apparmor !== "absent") stub("aa-enabled", `exit ${apparmor === "active" ? 0 : 1}`);
  return { root, tree, bin, log };
}

function run(h, script, arg) {
  assertRooted(script);
  const r = spawnSync("/bin/sh", [script, arg], {
    encoding: "utf8",
    env: { PATH: h.bin, DPKG_ROOT: h.tree },
  });
  const calls = fs.existsSync(h.log) ? fs.readFileSync(h.log, "utf8").split("\n").filter(Boolean) : [];
  return { ...r, calls };
}

const profileAt = (h) => path.join(h.tree, PROFILE);
const cleanup = (h) => fs.rmSync(h.root, { recursive: true, force: true });

test("install writes the profile and loads it where AppArmor is active", () => {
  const h = host();
  try {
    const r = run(h, POSTINST, "configure");
    assert.equal(r.status, 0, r.stderr);
    assert.ok(fs.existsSync(profileAt(h)), "profile written");
    assert.equal(fs.statSync(profileAt(h)).mode & 0o777, 0o644);
    assert.ok(r.calls.includes(`apparmor_parser -r -T -W ${profileAt(h)}`), r.calls.join("\n"));
  } finally {
    cleanup(h);
  }
});

// The profile is checked by AppArmor's own parser, not by pattern:
// it must parse against the host's AppArmor, attach to the installed
// binary and grant user namespaces. Where no parser is installed the
// test says so and stops short of that check.
test("the profile parses, attaches to the installed binary and grants userns", (t) => {
  const parser = ["/usr/sbin/apparmor_parser", "/sbin/apparmor_parser"].find((p) => fs.existsSync(p));
  const h = host();
  try {
    run(h, POSTINST, "configure");
    const text = fs.readFileSync(profileAt(h), "utf8");
    assert.match(text, /^profile ani-gui \/opt\/ani-gui\/ani-gui\.bin flags=\(unconfined\) \{$/m);
    assert.match(text, /^\s*userns,$/m);
    // As in Ubuntu's own profiles for Chrome, VS Code and Obsidian.
    assert.match(text, /^\s*@\{exec_path\} mr,$/m);
    if (!parser) {
      t.skip("apparmor_parser is not installed here: the profile was not parsed");
      return;
    }
    if (!fs.existsSync("/etc/apparmor.d/abi/4.0")) {
      t.skip("this host's AppArmor predates ABI 4.0, where the postinst writes no profile: not parsed");
      return;
    }
    const parsed = spawnSync(parser, ["-Q", "-K", "-T", "--skip-cache", profileAt(h)], { encoding: "utf8" });
    assert.equal(parsed.status, 0, parsed.stderr);
  } finally {
    cleanup(h);
  }
});

test("the setuid helper is still made SUID root, as the fallback", () => {
  for (const apparmor of ["active", "inactive", "absent"]) {
    const h = host({ apparmor });
    try {
      const r = run(h, POSTINST, "configure");
      assert.equal(r.status, 0, r.stderr);
      const sandbox = path.join(h.tree, "opt", "ani-gui", "chrome-sandbox");
      assert.ok(r.calls.includes(`chown root:root ${sandbox}`), apparmor);
      assert.equal(fs.statSync(sandbox).mode & 0o7777, 0o4755, apparmor);
    } finally {
      cleanup(h);
    }
  }
});

test("with AppArmor installed but not active, the profile is written and not loaded", () => {
  for (const apparmor of ["inactive", "absent"]) {
    const h = host({ apparmor });
    try {
      const r = run(h, POSTINST, "configure");
      assert.equal(r.status, 0, r.stderr);
      assert.ok(fs.existsSync(profileAt(h)), apparmor);
      assert.ok(!r.calls.some((c) => c.startsWith("apparmor_parser")), apparmor);
    } finally {
      cleanup(h);
    }
  }
});

// An AppArmor older than ABI 4.0 (Debian 12's 3.0) cannot parse the
// `userns` rule, and a profile it cannot parse would fail its boot-time
// load. Such hosts have no user-namespace restriction to lift, so the
// profile is left out entirely; so is it on hosts with no AppArmor.
test("a host whose AppArmor predates ABI 4.0, or that has none, gets no profile", () => {
  const h = host({ abi4: false });
  try {
    const r = run(h, POSTINST, "configure");
    assert.equal(r.status, 0, r.stderr);
    assert.ok(!fs.existsSync(profileAt(h)));
    assert.ok(!r.calls.some((c) => c.startsWith("apparmor_parser")));
  } finally {
    cleanup(h);
  }
});

test("a profile that fails to load never fails the install", () => {
  const h = host({ parser: 1 });
  try {
    const r = run(h, POSTINST, "configure");
    assert.equal(r.status, 0, r.stderr);
    assert.equal(fs.statSync(path.join(h.tree, "opt", "ani-gui", "chrome-sandbox")).mode & 0o7777, 0o4755);
  } finally {
    cleanup(h);
  }
});

test("the terminal launcher link is still made, inside the target root", () => {
  const h = host();
  try {
    run(h, POSTINST, "configure");
    assert.equal(fs.readlinkSync(path.join(h.tree, "usr", "local", "bin", "ani-gui")), "/opt/ani-gui/ani-gui");
  } finally {
    cleanup(h);
  }
});

// `disappear` is dpkg's word for a package whose every file another
// package has replaced: it is gone as surely as on remove.
for (const action of ["remove", "purge", "disappear"]) {
  test(`${action} unloads the profile and deletes it`, () => {
    const h = host();
    try {
      run(h, POSTINST, "configure");
      const cache = path.join(h.tree, "var", "cache", "apparmor", "abc123.0");
      fs.mkdirSync(cache, { recursive: true });
      fs.writeFileSync(path.join(cache, "ani-gui"), "");
      fs.rmSync(h.log, { force: true });
      const r = run(h, POSTRM, action);
      assert.equal(r.status, 0, r.stderr);
      assert.ok(r.calls.includes(`apparmor_parser -R ${profileAt(h)}`), r.calls.join("\n"));
      assert.ok(!fs.existsSync(profileAt(h)), "profile deleted");
      assert.ok(!fs.existsSync(path.join(cache, "ani-gui")), "its compiled cache deleted");
    } finally {
      cleanup(h);
    }
  });
}

test("remove where AppArmor is not active deletes the profile without unloading", () => {
  const h = host({ apparmor: "inactive" });
  try {
    run(h, POSTINST, "configure");
    fs.rmSync(h.log, { force: true });
    const r = run(h, POSTRM, "remove");
    assert.equal(r.status, 0, r.stderr);
    assert.ok(!fs.existsSync(profileAt(h)));
    assert.ok(!r.calls.some((c) => c.startsWith("apparmor_parser")));
  } finally {
    cleanup(h);
  }
});

// dpkg runs the old package's postrm with `upgrade` during an upgrade;
// the new postinst rewrites and reloads the profile right after, so
// unloading it here would only open a window without it.
test("an upgrade keeps the profile loaded", () => {
  const h = host();
  try {
    run(h, POSTINST, "configure");
    fs.rmSync(h.log, { force: true });
    const r = run(h, POSTRM, "upgrade");
    assert.equal(r.status, 0, r.stderr);
    assert.ok(fs.existsSync(profileAt(h)));
    assert.ok(!r.calls.some((c) => c.startsWith("apparmor_parser")));
  } finally {
    cleanup(h);
  }
});

test("remove on a host that never had the profile succeeds", () => {
  const h = host({ abi4: false });
  try {
    const r = run(h, POSTRM, "remove");
    assert.equal(r.status, 0, r.stderr);
    assert.ok(!r.calls.some((c) => c.startsWith("apparmor_parser")));
  } finally {
    cleanup(h);
  }
});

// electron-builder runs both scripts through a template step that
// replaces every `${name}` made of letters with one of its own values
// and stops the build on a name it does not know
// (app-builder-lib/out/targets/FpmTarget.js, writeConfigFile). A
// script that needs a braced expansion has to spell it some other way.
test("neither script carries a macro electron-builder would try to fill in", () => {
  for (const script of [POSTINST, POSTRM]) {
    const text = fs.readFileSync(script, "utf8");
    assert.deepEqual(text.match(/\$\{[a-zA-Z]+\}/g) ?? [], [], script);
    assert.deepEqual(text.match(/<%=[a-zA-Z]+%>/g) ?? [], [], script);
  }
});

// On purge, what a local administrator added for the profile goes with
// it, as dh_apparmor's postrm does: the local override and the
// disable / force-complain markers.
test("purge removes the profile's local override and markers", () => {
  const h = host();
  try {
    run(h, POSTINST, "configure");
    for (const dir of ["local", "disable", "force-complain"]) {
      fs.mkdirSync(path.join(h.tree, "etc", "apparmor.d", dir), { recursive: true });
      fs.writeFileSync(path.join(h.tree, "etc", "apparmor.d", dir, "ani-gui"), "");
    }
    run(h, POSTRM, "remove");
    for (const dir of ["local", "disable", "force-complain"]) {
      assert.ok(fs.existsSync(path.join(h.tree, "etc", "apparmor.d", dir, "ani-gui")), `remove keeps ${dir}/`);
    }
    const r = run(h, POSTRM, "purge");
    assert.equal(r.status, 0, r.stderr);
    for (const dir of ["local", "disable", "force-complain"]) {
      assert.ok(!fs.existsSync(path.join(h.tree, "etc", "apparmor.d", dir, "ani-gui")), `purge removes ${dir}/`);
    }
  } finally {
    cleanup(h);
  }
});
