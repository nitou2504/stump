#!/usr/bin/env python3
"""Tests of pdf_runner.py's queue protocol with fake jobs (no OCR, no docker):

    python3 scripts/fork/workers/test_pdf_runner.py
"""

import fcntl
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pdf_runner  # noqa: E402


class RunnerTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = Path(self.tmp.name)
        self.config = t / "config"
        self.books = t / "books"
        (self.books / "biblioteca/A_Author").mkdir(parents=True)
        self.pdf = self.books / "biblioteca/A_Author/Scan.pdf"
        self.pdf.write_bytes(b"%PDF-1.4 fake scan " * 100)
        self.root = self.config / "workers/pdf"
        for d in ("queue", "running", "done", "archive"):
            (self.root / d).mkdir(parents=True)
        self.env = dict(os.environ, STUMP_PATH_MAP=f"/config={self.config},/biblioteca={self.books}",
                        PDF_RUNNER_WORK=str(t / "work"))

    def tearDown(self):
        self.tmp.cleanup()

    def request(self, rid="t1", where="queue", **over):
        st = self.pdf.stat()
        req = {"version": 1, "id": rid, "steps": ["ocr", "shrink"],
               "path": "/biblioteca/biblioteca/A_Author/Scan.pdf", "library_path": "/biblioteca/biblioteca",
               "bytes": st.st_size, "mtime": int(st.st_mtime), "language": "es",
               "created_at": "2026-11-02T00:00:00Z"}
        req.update(over)
        (self.root / where / f"{rid}.json").write_text(json.dumps(req))
        return req

    def drain(self, fake="done"):
        env = dict(self.env, PDF_RUNNER_FAKE=fake)
        return subprocess.run([sys.executable, str(HERE / "pdf_runner.py"), "drain"], env=env,
                              capture_output=True, text=True)

    def result(self, rid="t1"):
        return json.loads((self.root / "done" / f"{rid}.json").read_text())

    def test_done_result_and_file(self):
        self.request()
        p = self.drain("done")
        self.assertEqual(p.returncode, 0, p.stderr)
        res = self.result()
        self.assertEqual(res["status"], "done")
        self.assertEqual(res["bytes_in"], self.pdf.stat().st_size)
        self.assertEqual((self.root / "done/t1.pdf").read_bytes(), self.pdf.read_bytes())
        self.assertEqual(list((self.root / "queue").iterdir()), [])
        self.assertEqual(list((self.root / "running").iterdir()), [])
        self.assertFalse(list((self.root / "done").glob(".*")), "no part files left")

    def test_not_worth_has_no_file(self):
        self.request()
        self.drain("not_worth")
        self.assertEqual(self.result()["status"], "not_worth")
        self.assertFalse((self.root / "done/t1.pdf").exists())

    def test_changed_file_is_refused(self):
        self.request(mtime=1)
        self.drain("done")
        res = self.result()
        self.assertEqual(res["status"], "failed")
        self.assertIn("changed", res["reason"])
        self.assertFalse((self.root / "done/t1.pdf").exists())

    def test_missing_file_and_unmapped_path(self):
        self.request("gone", path="/biblioteca/biblioteca/A_Author/Gone.pdf")
        self.request("elsewhere", path="/other/x.pdf")
        self.drain("done")
        self.assertIn("not in the library", self.result("gone")["reason"])
        self.assertIn("no mount", self.result("elsewhere")["reason"])

    def test_stale_running_request_is_picked_up(self):
        self.request(where="running")
        self.drain("done")
        self.assertEqual(self.result()["status"], "done")
        self.assertEqual(list((self.root / "running").iterdir()), [])

    def test_second_runner_backs_off(self):
        self.request()
        with (self.root / "runner.lock").open("a+") as fh:
            fcntl.flock(fh, fcntl.LOCK_EX | fcntl.LOCK_NB)
            p = self.drain("done")
        self.assertIn("another runner", p.stdout)
        self.assertTrue((self.root / "queue/t1.json").exists())

    def test_path_map_longest_prefix(self):
        pairs = sorted([("/biblioteca", "/mnt/b"), ("/biblioteca/x", "/mnt/x"), ("/config", "/srv/c")],
                       key=lambda p: -len(p[0]))
        self.assertEqual(pdf_runner.to_host("/biblioteca/x/y.pdf", pairs), Path("/mnt/x/y.pdf"))
        self.assertEqual(pdf_runner.to_host("/biblioteca/xy.pdf", pairs), Path("/mnt/b/xy.pdf"))
        with self.assertRaises(ValueError):
            pdf_runner.to_host("/bibliotecas/a.pdf", pairs)


if __name__ == "__main__":
    unittest.main(verbosity=2)
