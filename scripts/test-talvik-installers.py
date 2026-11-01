#!/usr/bin/env python3
"""Installer contract tests; fake payloads never start a real service."""
import json
import os
import pathlib
import plistlib
import shutil
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]


class InstallerTests(unittest.TestCase):
    def fixture(self, layout):
        temporary = tempfile.TemporaryDirectory(prefix="talvik-install-")
        self.addCleanup(temporary.cleanup)
        root = pathlib.Path(temporary.name)
        payload = root / "payload"
        scripts = payload if layout == "archive" else payload / "scripts"
        scripts.mkdir(parents=True)
        template = scripts if layout == "archive" else payload / "packaging/talvik"
        template.mkdir(parents=True, exist_ok=True)
        for name in ["install-model-runner.sh", "install-talvik-macos.sh"]:
            shutil.copy2(ROOT / "scripts" / name, scripts / name)
        shutil.copy2(ROOT / "packaging/talvik/talvikd.plist.in", template / "talvikd.plist.in")
        (payload / "bin").mkdir()
        (payload / "lib").mkdir()
        for name in ["talvik", "talvikd"]:
            p = payload / "bin" / name
            p.write_text("#!/bin/sh\necho fake-payload\n")
            p.chmod(0o755)
        for name in ["llama", "ggml", "ggml-base", "ggml-cpu", "mtmd", "oid-chat-template"]:
            p = payload / "lib" / f"lib{name}.1.dylib"
            p.write_bytes(b"fixture")
            p.with_name(f"lib{name}.dylib").symlink_to(p.name)
        prefix = root / "prefix with spaces & XML"
        agents = root / "LaunchAgents"
        mocks = root / "mocks"
        mocks.mkdir()
        launchctl = mocks / "launchctl"
        launchctl.write_text(
            "#!/usr/bin/env python3\nimport json,os,sys\n"
            "with open(os.environ['TALVIK_TEST_LAUNCH_LOG'],'a') as f:"
            " f.write(json.dumps(sys.argv[1:])+'\\n')\n"
        )
        launchctl.chmod(0o755)
        env = {**os.environ, "PATH": str(mocks) + os.pathsep + os.environ["PATH"],
               "TALVIK_HOME": str(prefix), "TALVIK_STATE_DIR": str(prefix),
               "TALVIK_MODEL_DIR": str(prefix / "models"), "LLAMA_CPP_LIB_DIR": "",
               "TALVIK_LAUNCH_AGENT_DIR": str(agents),
               "TALVIK_TEST_LAUNCH_LOG": str(root / "launch.log")}
        return scripts, prefix, agents, env

    def run_installer(self, scripts, prefix, env, *args):
        result = subprocess.run(["bash", str(scripts / "install-talvik-macos.sh"),
                                 "--prefix", str(prefix), *args], env=env,
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        return result

    def assert_service(self, prefix, agents):
        p = plistlib.loads((agents / "com.talvik.talvikd.plist").read_bytes())
        self.assertEqual(p["ProgramArguments"], [str(prefix.resolve() / "bin/talvikd")])
        self.assertTrue(p["RunAtLoad"])
        self.assertEqual(p["EnvironmentVariables"]["TALVIK_HTTP_ADDR"], "127.0.0.1:11435")
        self.assertEqual(p["EnvironmentVariables"]["TALVIK_STATE_DIR"], str(prefix.resolve()))
        self.assertTrue((prefix / "lib/libmtmd.dylib").is_file())
        self.assertTrue((prefix / "lib/libmtmd.dylib").is_symlink())
        self.assertTrue((prefix / "lib/liboid-chat-template.dylib").is_file())
        self.assertTrue((prefix / "lib/liboid-chat-template.dylib").is_symlink())

    def test_archive_install_preserves_native_vision_library_and_escaped_paths(self):
        scripts, prefix, agents, env = self.fixture("archive")
        result = self.run_installer(scripts, prefix, env, "--service")
        self.assert_service(prefix, agents)
        self.assertIn("Installed and started Talvik", result.stdout)
        calls = [json.loads(line) for line in pathlib.Path(env["TALVIK_TEST_LAUNCH_LOG"]).read_text().splitlines()]
        self.assertEqual([call[0] for call in calls], ["bootout", "bootstrap"])

    def test_source_layout_resolves_packaging_template(self):
        scripts, prefix, agents, env = self.fixture("source")
        self.run_installer(scripts, prefix, env, "--service")
        self.assert_service(prefix, agents)

    def test_incomplete_native_payload_is_rejected_before_binary_install(self):
        scripts, prefix, agents, env = self.fixture("archive")
        for p in (scripts / "lib").glob("libmtmd*"):
            p.unlink()
        result = subprocess.run(["bash", str(scripts / "install-model-runner.sh"),
                                 "--prefix", str(prefix)], env=env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("libmtmd is missing", result.stderr)
        self.assertFalse((prefix / "bin/talvikd").exists())

    def test_missing_chat_template_renderer_is_rejected_before_binary_install(self):
        scripts, prefix, agents, env = self.fixture("archive")
        for p in (scripts / "lib").glob("liboid-chat-template*"):
            p.unlink()
        result = subprocess.run(["bash", str(scripts / "install-model-runner.sh"),
                                 "--prefix", str(prefix)], env=env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("liboid-chat-template is missing", result.stderr)
        self.assertFalse((prefix / "bin/talvikd").exists())

    def test_release_payload_is_not_overwritten_by_host_library_environment(self):
        scripts, prefix, agents, env = self.fixture("archive")
        host = prefix.parent / "host-lib"
        host.mkdir()
        (host / "libllama.dylib").write_bytes(b"unrelated host library")
        env["LLAMA_CPP_LIB_DIR"] = str(host)
        self.run_installer(scripts, prefix, env)
        self.assertEqual((prefix / "lib/libllama.1.dylib").read_bytes(), b"fixture")

    def test_upgrade_and_uninstall_preserve_models_and_registry(self):
        scripts, prefix, agents, env = self.fixture("archive")
        self.run_installer(scripts, prefix, env, "--service")
        model = prefix / "models/keep.gguf"
        registry = prefix / "models.registry"
        model.write_bytes(b"user model")
        registry.write_text("user registry")
        self.run_installer(scripts, prefix, env, "--service")
        self.run_installer(scripts, prefix, env, "--service", "--uninstall")
        self.assertEqual(model.read_bytes(), b"user model")
        self.assertEqual(registry.read_text(), "user registry")
        self.assertFalse((prefix / "bin/talvikd").exists())
        self.assertFalse((agents / "com.talvik.talvikd.plist").exists())
        self.assertFalse(list((prefix / "lib").glob("libmtmd*")) if (prefix / "lib").exists() else [])
        self.assertFalse(list((prefix / "lib").glob("liboid-chat-template*")) if (prefix / "lib").exists() else [])

    def test_unix_release_packager_includes_required_chat_template_renderer(self):
        with tempfile.TemporaryDirectory(prefix="talvik-package-test-") as directory:
            root = pathlib.Path(directory)
            binaries = root / "bin"
            libraries = root / "lib"
            binaries.mkdir()
            libraries.mkdir()
            for name in ["talvik", "talvikd"]:
                executable = binaries / name
                executable.write_text("#!/bin/sh\nprintf '{}\\n'\n")
                executable.chmod(0o755)
            for name in ["llama", "ggml", "ggml-base", "ggml-cpu", "mtmd", "oid-chat-template"]:
                (libraries / f"lib{name}.dylib").write_bytes(b"fixture")
            archive = root / "talvik.tar.gz"
            result = subprocess.run(
                ["bash", str(ROOT / "scripts/package-talvik-unix.sh"),
                 str(binaries), str(libraries), str(archive)],
                capture_output=True, text=True, timeout=30
            )
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
            listing = subprocess.run(
                ["tar", "-tzf", str(archive)],
                capture_output=True, text=True, check=True
            ).stdout.splitlines()
            self.assertIn("talvik/lib/liboid-chat-template.dylib", listing)
            self.assertTrue(pathlib.Path(f"{archive}.sha256").is_file())


if __name__ == "__main__":
    unittest.main()
