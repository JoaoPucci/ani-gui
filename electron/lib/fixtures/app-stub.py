#!/usr/bin/env python3
"""Stands in for the app's binary (ani-gui.bin) behind the Linux launcher.

linux-launcher.test.cjs writes the launcher next to this file under the
binary's name, runs it, and reads what happened from the events file.

Like the app's main process, it writes to its stdout and stderr, and it
answers the first SIGTERM, SIGINT or SIGHUP by shutting down: the signal's
handler goes back to the default as it runs, as Electron's does, so a second
one of the same kind ends it on the spot. A failed write is noted and
otherwise ignored, as lib/stdio-guard.cjs does for the app.

Environment:
    STUB_EVENTS      the events file, one JSON object per line (required)
    STUB_LINES       lines written to stdout ("out N") and stderr ("err N")
                     at start (default 3)
    STUB_MODE        wait: run until a stop signal (default)
                     exit: write the lines and exit with STUB_EXIT
                     die:  write the lines and die by SIGUSR1
    STUB_EXIT        the exit status (default 0)
    STUB_BYE_LINES   lines written to each stream after a stop signal
                     ("bye N", default 3)
    STUB_HOLD_MS     as it exits, leave a child holding its stdout and
                     stderr for this long, as Chromium's helpers or a
                     restarted app do (default 0: none)

Events: {"start": {...}} once, then {"ready": true} once the handlers are in
place, {"signal": "TERM"} per stop signal, {"write_error": "EPIPE"} at the
first failed write, {"exit": N} as it exits. It exits by itself after 30 s,
so a test that fails leaves nothing running.
"""
import errno
import json
import os
import signal
import sys
import time

EVENTS = os.environ["STUB_EVENTS"]
failed = False
stopping = []


def event(obj):
    fd = os.open(EVENTS, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
    try:
        os.write(fd, (json.dumps(obj) + "\n").encode())
    finally:
        os.close(fd)


def put(fd, text):
    global failed
    try:
        os.write(fd, (text + "\n").encode())
    except OSError as err:
        if not failed:
            failed = True
            event({"write_error": errno.errorcode.get(err.errno, str(err.errno))})


def stop(signum, _frame):
    signal.signal(signum, signal.SIG_DFL)
    event({"signal": signal.Signals(signum).name[3:]})
    stopping.append(signum)


def hold():
    """Leaves a child with this process's stdout and stderr open for
    STUB_HOLD_MS, which then exits by itself."""
    ms = int(os.environ.get("STUB_HOLD_MS", "0"))
    if ms <= 0:
        return
    if os.fork() == 0:
        time.sleep(ms / 1000)
        os._exit(0)


def target(fd):
    try:
        return os.readlink(f"/proc/self/fd/{fd}")
    except OSError:
        return None


def main():
    event({"start": {
        "pid": os.getpid(),
        "pgid": os.getpgid(0),
        "sid": os.getsid(0),
        "argv": sys.argv[1:],
        "hint": os.environ.get("ELECTRON_OZONE_PLATFORM_HINT"),
        "tty": [os.isatty(fd) for fd in (0, 1, 2)],
        "stdin": target(0),
    }})
    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(sig, stop)
    lines = int(os.environ.get("STUB_LINES", "3"))
    for n in range(1, lines + 1):
        put(1, f"out {n}")
        put(2, f"err {n}")
    mode = os.environ.get("STUB_MODE", "wait")
    status = int(os.environ.get("STUB_EXIT", "0"))
    if mode == "die":
        event({"exit": "SIGUSR1"})
        os.kill(os.getpid(), signal.SIGUSR1)
    if mode == "exit":
        event({"exit": status})
        return status
    event({"ready": True})
    deadline = time.monotonic() + 30
    while not stopping and time.monotonic() < deadline:
        time.sleep(0.02)
    if not stopping:
        event({"exit": 99})
        return 99
    for n in range(1, int(os.environ.get("STUB_BYE_LINES", "3")) + 1):
        put(1, f"bye {n}")
        put(2, f"bye-err {n}")
    # Shutting down takes a moment, as the app's does: a second signal
    # of the same kind in that window ends it on the spot.
    time.sleep(0.5)
    hold()
    event({"exit": status})
    return status


if __name__ == "__main__":
    sys.exit(main())
