#!/usr/bin/env python3
"""Unix acceptance checks for OID's dot prompt, footer and terminal restoration.
Usage: python3 scripts/test-oid-console-terminal.py [BIN_DIR]
"""
import fcntl
import importlib.util
import os
import pathlib
import pty
import re
import select
import signal
import socket
import struct
import subprocess
import sys
import termios
import time
import unittest

# Establish a controlling terminal after exec, avoiding a Python preexec_fn
# in a multithreaded fixture process.
if __name__ == "__main__" and len(sys.argv) > 1 and sys.argv[1] == "--child":
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)
    native = os.environ.get("OID_TEST_NATIVE_LIB_DIR")
    if native:
        os.environ["DYLD_LIBRARY_PATH"] = native
        os.environ["LD_LIBRARY_PATH"] = native
    os.execve(sys.argv[2], sys.argv[2:], os.environ)

ROOT = pathlib.Path(__file__).resolve().parents[1]
BIN = pathlib.Path(sys.argv.pop(1) if len(sys.argv) > 1 else ROOT / "target/debug").resolve()
_spec = importlib.util.spec_from_file_location("terminal_fixture", ROOT / "scripts/test-talvik-terminal.py")
fixture = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(fixture)
ANSI = re.compile(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|[78])")


class Console:
    def __init__(self, env, columns=80, rows=24, binary=None):
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        self.original = termios.tcgetattr(self.slave)
        self.data = bytearray()
        native = os.environ.get("TALVIK_TEST_NATIVE_LIB_DIR")
        if native:
            env = {**env, "OID_TEST_NATIVE_LIB_DIR": native}
        self.process = subprocess.Popen(
            [sys.executable, str(pathlib.Path(__file__).resolve()), "--child", str(binary or BIN / "oid-console")],
            stdin=self.slave, stdout=self.slave, stderr=self.slave,
            env=env, start_new_session=True,
        )

    def read_more(self, seconds=0.1):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([self.master], [], [], min(0.05, max(0, deadline-time.monotonic())))[0]:
                data = os.read(self.master, 65536)
                if not data:
                    break
                self.data.extend(data)

    def until(self, needle, seconds=5):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if needle in self.text():
                return
            self.read_more()
            if self.process.poll() is not None:
                break
        raise AssertionError(f"Did not receive {needle!r}; output: {self.text()[-1600:]!r}")

    def text(self):
        return self.data.decode(errors="replace")

    def plain(self):
        return ANSI.sub("", self.text())

    def send(self, data):
        os.write(self.master, data if isinstance(data, bytes) else data.encode())

    def resize(self, columns, rows):
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        self.process.send_signal(signal.SIGWINCH)

    def wait(self):
        deadline = time.monotonic() + 5
        while self.process.poll() is None and time.monotonic() < deadline:
            self.read_more()
        result = self.process.wait(timeout=1)
        self.read_more()
        return result

    def restored(self):
        return termios.tcgetattr(self.master) == self.original

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
            self.process.wait()
        os.close(self.master)
        os.close(self.slave)


class ConsoleTests(unittest.TestCase):
    def start(self, daemon, **overrides):
        env = daemon.environment(NO_COLOR="1", OID_HISTORY_FILE=str(daemon.root / "history"), **overrides)
        console = Console(env)
        self.addCleanup(console.close)
        console.until("\x1b[?2004h")
        return console

    def test_clean_header_dot_prompt_and_one_reserved_footer(self):
        with fixture.daemon() as daemon:
            console = self.start(daemon)
            console.until(". \x1b[3G")
            self.assertIn("OID Console", console.plain())
            self.assertNotRegex(console.text(), r"\x1b\[[0-9;]*m")
            self.assertIn("Ready · No model loaded", console.plain())
            self.assertIn("\x1b[1;23r", console.text())
            self.assertIn("\x1b[24;1H", console.text())
            for old in ["Available models:", "Intelligent Runtime", "int>", "▋", "[OID Healthy"]:
                self.assertNotIn(old, console.plain())
            console.send(":status\r")
            console.until("Runtime : Healthy")
            console.send(":quit\r")
            self.assertEqual(console.wait(), 0)
            self.assertIn("\x1b[r", console.text())
            self.assertTrue(console.restored())

    def test_resize_and_long_line_do_not_wrap_the_prompt(self):
        with fixture.daemon() as daemon:
            console = self.start(daemon)
            console.send("a" * 140)
            console.read_more(0.25)
            console.resize(32, 12)
            console.until("\x1b[1;11r")
            console.read_more(0.15)
            lines = re.findall(r"\r\x1b\[2K(\. [^\r\n]*?)\x1b\[\d+G", console.text())
            self.assertTrue(lines, console.text())
            self.assertLess(len(lines[-1]), 32)
            self.assertIn("<", lines[-1])
            console.send(b"\x03")
            console.until("^C")
            console.send(":quit\r")
            self.assertEqual(console.wait(), 0)
            self.assertTrue(console.restored())

    def test_native_shell_handoff_and_unicode_input(self):
        with fixture.daemon() as daemon:
            console = self.start(daemon)
            console.send("printf 'SHELL_OK\\n'\r")
            console.until("SHELL_OK\r\n")
            console.send("printf '界é\\n'\r")
            console.until("界é\r\n")
            console.send(":quit\r")
            self.assertEqual(console.wait(), 0)
            self.assertGreaterEqual(console.text().count("\x1b[r"), 3)
            self.assertTrue(console.restored())

    def test_generation_returns_to_a_clean_footer(self):
        with fixture.daemon() as daemon:
            console = self.start(daemon)
            console.send(":generate hello\r")
            console.until("Complete")
            console.read_more(0.2)
            self.assertIn("Thinking", console.plain())
            self.assertIn("Generating", console.plain())
            self.assertNotIn("DONE", console.plain())
            console.send(":quit\r")
            self.assertEqual(console.wait(), 0)
            self.assertTrue(console.restored())

    def test_termination_while_editing_restores_the_terminal(self):
        with fixture.daemon() as daemon:
            console = self.start(daemon)
            console.send("unfinished command")
            console.process.send_signal(signal.SIGTERM)
            self.assertEqual(console.wait(), 0)
            self.assertTrue(console.restored())
            self.assertIn("Console closed.", console.plain())

    def test_hidden_status_still_has_a_dot_prompt(self):
        with fixture.daemon() as daemon:
            console = self.start(daemon, OID_STATUS_MODE="hidden")
            console.until(". \x1b[3G")
            self.assertNotIn("\x1b[1;23r", console.text())
            console.send(b"\x04")
            self.assertEqual(console.wait(), 0)
            self.assertTrue(console.restored())

    def test_piped_input_stays_plain_and_keeps_explicit_diagnostics(self):
        with fixture.daemon() as daemon:
            env = daemon.environment(OID_HISTORY_FILE=str(daemon.root / "history"))
            native = os.environ.get("TALVIK_TEST_NATIVE_LIB_DIR")
            if native:
                env["DYLD_LIBRARY_PATH"] = native
                env["LD_LIBRARY_PATH"] = native
            result = subprocess.run([str(BIN / "oid-console")], input=":status\n:clear\n:quit\n", env=env,
                                    text=True, capture_output=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertNotIn("\x1b", result.stdout)
            self.assertIn("Runtime : Healthy", result.stdout)
            self.assertNotIn("Available models:", result.stdout)


if __name__ == "__main__":
    unittest.main()
