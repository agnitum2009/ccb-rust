"""Validation test for Rust-aligned release artifacts.

This test builds a Linux release artifact (preview/dirty is acceptable) and
verifies that it contains the Rust binaries required by install.sh.

Can be run directly with Python or via pytest if pytest is installed.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
RUST_ROOT = REPO_ROOT / "rust"
REQUIRED_BINARIES = ("ccbr", "ccbd", "ask", "autonew", "ctx-transfer")


class TestLinuxReleaseArtifact(unittest.TestCase):
    @unittest.skipUnless(sys.platform == "linux", "release artifact validation only runs on Linux")
    @unittest.skipUnless(shutil.which("cargo") is not None, "cargo not available")
    def test_linux_release_artifact_contains_rust_binaries(self) -> None:
        """Build a preview release artifact and assert required Rust binaries are packaged."""
        output_dir = Path(tempfile.mkdtemp(prefix="ccb-release-validation-"))
        try:
            result = subprocess.run(
                [
                    sys.executable,
                    str(REPO_ROOT / "scripts" / "build_linux_release.py"),
                    "--allow-dirty",
                    "--output-dir",
                    str(output_dir),
                ],
                cwd=REPO_ROOT,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(
                result.returncode,
                0,
                f"Release build failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )

            tar_files = list(output_dir.glob("*.tar.gz"))
            self.assertTrue(tar_files, f"No tarball produced in {output_dir}")
            artifact = tar_files[0]

            extract_dir = output_dir / "extracted"
            extract_dir.mkdir()
            with tarfile.open(artifact, "r:gz") as tar:
                tar.extractall(extract_dir)

            artifact_roots = [p for p in extract_dir.iterdir() if p.is_dir()]
            self.assertTrue(artifact_roots, "No artifact root directory in tarball")
            root = artifact_roots[0]

            build_info_path = root / "BUILD_INFO.json"
            self.assertTrue(build_info_path.is_file(), "BUILD_INFO.json missing from artifact")
            build_info = json.loads(build_info_path.read_text(encoding="utf-8"))
            for key in ("version", "platform", "arch", "channel"):
                self.assertTrue(build_info.get(key), f"BUILD_INFO.json missing {key}")
            self.assertEqual(build_info["platform"], "linux")

            for name in REQUIRED_BINARIES:
                binary = root / "bin" / name if name != "ccbr" else root / name
                self.assertTrue(binary.is_file(), f"Required binary missing: {binary}")
                self.assertTrue(
                    os.access(binary, os.X_OK),
                    f"Required binary not executable: {binary}",
                )

            ccbr_binary = root / "ccbr"
            version_result = subprocess.run(
                [str(ccbr_binary), "version"],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(
                version_result.returncode,
                0,
                f"ccb version failed: {version_result.stderr}",
            )
            self.assertIn("7.5", version_result.stdout)

            for name in ("ask", "autonew", "ctx-transfer"):
                help_result = subprocess.run(
                    [str(root / "bin" / name), "--help"],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(
                    help_result.returncode,
                    0,
                    f"{name} --help failed: {help_result.stderr}",
                )
        finally:
            shutil.rmtree(output_dir, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()
