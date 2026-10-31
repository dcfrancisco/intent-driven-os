#!/usr/bin/env python3
"""Unix terminal acceptance checks against built clients and a local fake daemon.

Usage: python3 scripts/test-talvik-terminal.py [BIN_DIR]
No model, token, network service or third-party Python package is required.
"""
import contextlib
import errno
import os
import pathlib
import pty
import select
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest

BIN = pathlib.Path(sys.argv.pop(1) if len(sys.argv) > 1 else "target/debug").resolve()
TEXT = '<think>Checking the result.</think>\n# Answer\n**Ready** with `code`.\n```python\nprint("ok")\n```\n'


def encode(value):
    return value.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r")


class Daemon:
    def __init__(self, cancel=False, fail=False):
        self.temporary = tempfile.TemporaryDirectory(prefix="tv-ui-", dir="/tmp")
        self.root = pathlib.Path(self.temporary.name)
        self.endpoint = str(self.root / "ipc.sock")
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.bind(self.endpoint)
        self.socket.listen()
        self.socket.settimeout(0.05)
        self.stop = threading.Event()
        self.started = threading.Event()
        self.cancelled = threading.Event()
        self.wait_for_cancel = cancel
        self.fail = fail
        self.workers = []
        self.worker = threading.Thread(target=self.serve, daemon=True)
        self.worker.start()

    def serve(self):
        while not self.stop.is_set():
            try:
                connection, _ = self.socket.accept()
            except socket.timeout:
                continue
            except OSError:
                return
            worker = threading.Thread(target=self.respond, args=(connection,), daemon=True)
            self.workers.append(worker)
            worker.start()

    def respond(self, connection):
        with connection:
            connection.settimeout(5)
            with connection.makefile("rwb") as stream:
                command = stream.readline().decode().split("\t")[0].strip()
                if command == "hello":
                    stream.write(b"OK protocol\t3\n")
                elif command == "status":
                    stream.write(b"OK status\tHealthy\tNone\t\t\t0.1\tmock\tfalse\ttest\ttest\tfalse\t0 B\t0\t0\t8\tfalse\t0\t0\t\t\tfalse\t\t\n")
                elif command == "model_list":
                    stream.write(b"END\n")
                elif command == "cancel":
                    self.cancelled.set()
                    stream.write(b"OK cancelled\n")
                elif command.startswith("generate"):
                    self.started.set()
                    if self.wait_for_cancel:
                        if not self.cancelled.wait(5):
                            return
                        state = "cancelled"
                    else:
                        time.sleep(0.25)
                        if self.fail:
                            stream.write(b"ERROR\tfixture model failed\n")
                            stream.flush()
                            return
                        # Exercise markup split at every possible character boundary.
                        for character in TEXT:
                            stream.write(("TOKEN\t" + encode(character) + "\n").encode())
                            stream.flush()
                        state = "completed"
                    stream.write((f"DONE\t{state}\tmock-generation\t28\t12\t40\t250\t240\t230\t200\t180\t30\t12.5\ttrue\tstop\tfalse\t0\tcpu\tCPU\t0\t0\t\n").encode())
                else:
                    stream.write(b"ERROR\tunsupported fixture command\n")
                stream.flush()

    def environment(self, **overrides):
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(("TALVIK_", "MARINA_", "OID_")) and key != "NO_COLOR"}
        native = os.environ.get("TALVIK_TEST_NATIVE_LIB_DIR")
        if native:
            env["DYLD_LIBRARY_PATH"] = native
            env["LD_LIBRARY_PATH"] = native
        return {**env, "TERM": "xterm-256color", "COLUMNS": "80",
                "TALVIK_HOME": str(self.root), "TALVIK_CONFIG": str(self.root / "config.yaml"),
                "TALVIK_SOCKET": self.endpoint, "OID_MARINA_MODEL": "fixture-model", **overrides}

    def close(self):
        self.stop.set()
        self.cancelled.set()
        self.socket.close()
        self.worker.join(1)
        for worker in self.workers:
            worker.join(1)
        self.temporary.cleanup()


@contextlib.contextmanager
def daemon(**kwargs):
    instance = Daemon(**kwargs)
    try:
        yield instance
    finally:
        instance.close()


def terminal(command, instance, interrupt=False, **env):
    master, slave = pty.openpty()
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=slave,
                               stderr=slave, env=instance.environment(**env), start_new_session=True)
    os.close(slave)
    output = bytearray()
    interrupted = False
    deadline = time.monotonic() + 8
    try:
        while time.monotonic() < deadline:
            if interrupt and instance.started.is_set() and not interrupted:
                process.send_signal(signal.SIGINT)
                interrupted = True
            if select.select([master], [], [], 0.05)[0]:
                try:
                    part = os.read(master, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        break
                    raise
                if not part:
                    break
                output.extend(part)
            elif process.poll() is not None:
                break
        else:
            raise AssertionError("terminal client did not exit within 8 seconds")
        return process.wait(timeout=2), output.decode().replace("\r\n", "\n")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)


class TerminalTests(unittest.TestCase):
    def test_color_streaming_thinking_code_and_completion(self):
        with daemon() as instance:
            code, text = terminal([str(BIN / "talvik"), "run", "fixture-model", "hello"], instance)
        self.assertEqual(code, 0, text)
        for expected in ["Waiting for model", "Thinking", "Generating", "Complete", "28 tokens", "12.5 tok/s", "\x1b[36m", "\x1b[1m"]:
            self.assertIn(expected, text)
        self.assertNotIn("<think>", text)
        self.assertNotIn("DONE", text)
        self.assertEqual(text.count("Ready"), 1)
        self.assertIn("\r\x1b[2K", text)

    def test_no_color_retains_labels_without_sgr(self):
        with daemon() as instance:
            code, text = terminal([str(BIN / "talvik"), "run", "fixture-model", "hello"], instance, NO_COLOR="1")
        self.assertEqual(code, 0, text)
        self.assertIn("Thinking", text)
        self.assertIn("Complete", text)
        self.assertNotRegex(text, r"\x1b\[[0-9;]*m")

    def test_plain_and_dumb_terminal_preserve_raw_model_text(self):
        for flags, env in [(["--plain"], {}), ([], {"TERM": "dumb"})]:
            with self.subTest(flags=flags, env=env), daemon() as instance:
                code, text = terminal([str(BIN / "talvik"), *flags, "run", "fixture-model", "hello"], instance, **env)
                self.assertEqual(code, 0, text)
                self.assertEqual(text, TEXT + "\n")

    def test_pipes_contain_one_answer_without_decoration_or_protocol(self):
        for binary, args in [("talvik", ["run", "fixture-model", "hello"]), ("oid-marina-chat", ["hello"])]:
            with self.subTest(binary=binary), daemon() as instance:
                result = subprocess.run([str(BIN / binary), *args], env=instance.environment(), capture_output=True, text=True, timeout=8)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, TEXT + "\n")
                self.assertEqual(result.stderr, "")

    def test_interrupt_before_first_token_cancels_both_clients(self):
        for binary, args in [("talvik", ["run", "fixture-model", "hello"]), ("oid-marina-chat", ["hello"])]:
            with self.subTest(binary=binary), daemon(cancel=True) as instance:
                _, text = terminal([str(BIN / binary), *args], instance, interrupt=True)
                self.assertTrue(instance.cancelled.is_set(), text)
                self.assertIn("Cancelled", text)
                self.assertNotIn("Complete", text)
                self.assertNotIn("Generating", text)

    def test_failure_clears_progress_and_never_claims_completion(self):
        with daemon(fail=True) as instance:
            code, text = terminal([str(BIN / "talvik"), "run", "fixture-model", "hello"], instance)
        self.assertNotEqual(code, 0)
        self.assertIn("Failed", text)
        self.assertIn("fixture model failed", text)
        self.assertNotIn("Complete", text)
        self.assertIn("\r\x1b[2K", text)


if __name__ == "__main__":
    unittest.main()
