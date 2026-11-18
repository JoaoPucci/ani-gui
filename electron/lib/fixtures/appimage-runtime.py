#!/usr/bin/env python3
"""Stands in for the AppImage runtime in front of the Linux launcher.

    appimage-runtime.py <mounted-file> <program> [args...]

The AppImage's files live on a FUSE mount, which the runtime holds only
while something holds its keepalive pipe: before it starts AppRun, it
makes a pipe, leaves its FUSE process writing to one end, and passes the
other to AppRun, which every process started under it inherits on the
same descriptor (fd 3, the first one past stdin, stdout and stderr).
Once no process holds that end, the next write fails and the runtime
unmounts. A process that closes fd 3 because it took it for its own lets
go of the mount; once every process has, the files under the mount are
gone, the app's binary among them.
(github.com/AppImage/type2-runtime, src/runtime/runtime.c: keepalive_pipe,
write_pipe_thread.)

Here the "mount" is <mounted-file>: once nothing holds the end passed on
fd 3, it is deleted. <program> runs in this process, with that end on fd 3,
as AppRun runs in the runtime's. The watcher gives up after 60 s, so a
test that fails leaves nothing running.
"""
import os
import signal
import sys
import time


def main():
    mounted, program = sys.argv[1], sys.argv[2:]
    read_end, write_end = os.pipe()
    if os.fork() == 0:
        # The FUSE process: a daemon of its own, out of the session and
        # group the program runs in.
        os.setsid()
        os.close(read_end)
        signal.signal(signal.SIGPIPE, signal.SIG_IGN)
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            try:
                os.write(write_end, b"\0")
            except OSError:
                # Nothing holds the keepalive end any more: unmount.
                try:
                    os.unlink(mounted)
                except FileNotFoundError:
                    pass
                break
            time.sleep(0.005)
        os._exit(0)
    os.close(write_end)
    if read_end != 3:
        os.dup2(read_end, 3)
        os.close(read_end)
    os.set_inheritable(3, True)
    os.execvp(program[0], program)


if __name__ == "__main__":
    main()
