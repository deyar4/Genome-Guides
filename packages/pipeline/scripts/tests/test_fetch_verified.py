"""Tests for scripts/common/fetch_verified.py using file:// URLs (no network). Run: python3 -m unittest <this file>."""
import gzip
import hashlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "common" / "fetch_verified.py"
PAYLOAD = b">c\nACGT\n" * 100


def run(url, sha, out, gunzip=False):
    cmd = [sys.executable, str(SCRIPT), "--url", url, "--sha256", sha, "--output", str(out), "--retries", "1"]
    if gunzip:
        cmd.append("--gunzip")
    return subprocess.run(cmd, capture_output=True, text=True)


class FetchVerified(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.gz = self.tmp / "ref.fa.gz"
        self.gz.write_bytes(gzip.compress(PAYLOAD))
        self.sha = hashlib.sha256(PAYLOAD).hexdigest()

    def test_ok_and_gunzip(self):
        out = self.tmp / "sub" / "ref.fa"
        r = run(self.gz.as_uri(), self.sha, out, gunzip=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(out.read_bytes(), PAYLOAD)
        self.assertFalse(Path(str(out) + ".part").exists())

    def test_hash_mismatch_leaves_no_output(self):
        out = self.tmp / "ref.fa"
        r = run(self.gz.as_uri(), "0" * 64, out, gunzip=True)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("sha256 mismatch", r.stderr + r.stdout)
        self.assertFalse(out.exists())
        self.assertTrue(Path(str(out) + ".rejected").exists())

    def test_truncated_gzip_leaves_no_output(self):
        trunc = self.tmp / "trunc.fa.gz"
        trunc.write_bytes(self.gz.read_bytes()[:-20])
        out = self.tmp / "ref.fa"
        r = run(trunc.as_uri(), self.sha, out, gunzip=True)
        self.assertNotEqual(r.returncode, 0)
        self.assertFalse(out.exists())

    def test_missing_source_fails(self):
        out = self.tmp / "ref.fa"
        r = run((self.tmp / "nope.gz").as_uri(), self.sha, out)
        self.assertNotEqual(r.returncode, 0)
        self.assertFalse(out.exists())


if __name__ == "__main__":
    unittest.main()
