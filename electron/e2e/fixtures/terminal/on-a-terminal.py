#!/usr/bin/env python3
"""Run a program on a terminal of its own, and close that terminal when told.

    on-a-terminal.py <program> [args...]

The program runs on a pseudo-terminal, as it would started from a terminal
window: its stdin, stdout and stderr are the terminal, which is also its
controlling terminal, and it leads the terminal's session. What it writes
there is relayed to this script's stderr.

This script's stdin takes one command per line:

    hangup  close the terminal, as closing the terminal window does: the
            program gets its SIGHUP, and every write it makes to the
            terminal from then on fails (EIO);
    kill    SIGKILL the program's process group.

On its stdout, one JSON object per line: {"pid": <pid>} once the program has
started, then {"exit": <code>} or {"signal": <number>} when it ends, and this
script exits. If this script's stdin closes first, it kills the program's
process group, so a test that dies leaves nothing behind.

The program is this script's child, so its pid, and the id of the process
group it leads, stay its own until this script reaps it: the group is
signalled only before that. Core dumps are off for the program.
"""
import json
import os
import pty
import resource
import select
import signal
import sys


def report(event):
    sys.stdout.write(json.dumps(event) + "\n")
    sys.stdout.flush()


def main():
    program = sys.argv[1:]
    if not program:
        sys.stderr.write(__doc__)
        return 2
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    pid, terminal = pty.fork()
    if pid == 0:
        try:
            os.execvp(program[0], program)
        finally:
            os._exit(127)
    report({"pid": pid})

    def kill_group():
        try:
            os.killpg(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass

    commands = sys.stdin.fileno()
    pending = b""
    # Stops once the terminal reads nothing more: every holder of its other
    # end has closed it.
    relaying = True
    while True:
        done, status = os.waitpid(pid, os.WNOHANG)
        if done:
            break
        watched = [commands] + ([terminal] if terminal is not None and relaying else [])
        ready, _, _ = select.select(watched, [], [], 0.1)
        if terminal is not None and terminal in ready:
            try:
                chunk = os.read(terminal, 65536)
            except OSError:
                chunk = b""
            if chunk:
                os.write(sys.stderr.fileno(), chunk)
            else:
                relaying = False
        if commands in ready:
            chunk = os.read(commands, 4096)
            if not chunk:
                kill_group()
                os.waitpid(pid, 0)
                return 1
            pending += chunk
            while b"\n" in pending:
                line, pending = pending.split(b"\n", 1)
                command = line.strip()
                if command == b"hangup" and terminal is not None:
                    os.close(terminal)
                    terminal = None
                elif command == b"kill":
                    kill_group()
    if os.WIFSIGNALED(status):
        report({"signal": os.WTERMSIG(status)})
    else:
        report({"exit": os.WEXITSTATUS(status)})
    return 0


if __name__ == "__main__":
    sys.exit(main())
