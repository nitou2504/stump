#!/usr/bin/env python3
"""Tests of the PDF worker's queue protocol, heartbeat and watcher with fake jobs (no OCR):

    python3 scripts/fork/workers/test_pdf_runner.py
    docker run --rm --entrypoint python3 stump-pdf-worker:<rev> /opt/stump-pdf-worker/test_pdf_runner.py
"""

import fcntl
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
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
                        PDF_RUNNER_WORK=str(t / "work"), G3_SLOTS_DIR=str(t / "no-slots"),
                        PDF_WORKER_VERSION="test-rev")

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

    def drain(self, fake="done", **env):
        env = dict(self.env, PDF_RUNNER_FAKE=fake, **env)
        return subprocess.run([sys.executable, str(HERE / "pdf_runner.py"), "drain"], env=env,
                              capture_output=True, text=True)

    def result(self, rid="t1"):
        return json.loads((self.root / "done" / f"{rid}.json").read_text())

    def heartbeat(self):
        return json.loads((self.root / "worker.json").read_text())

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

    def test_pause_file_keeps_the_queue(self):
        self.request()
        (self.root / "PAUSE").touch()
        p = self.drain("done")
        self.assertIn("paused", p.stdout)
        self.assertTrue((self.root / "queue/t1.json").exists())
        self.assertFalse((self.root / "done/t1.json").exists())

    def test_parallel_jobs_handle_every_request(self):
        for i in range(5):
            self.request(f"r{i}")
        p = self.drain("done", PDF_WORKER_JOBS="2")
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertIn("(5 handled)", p.stdout)
        for i in range(5):
            self.assertEqual(self.result(f"r{i}")["status"], "done")

    def test_drain_leaves_an_idle_heartbeat(self):
        self.request()
        self.drain("done")
        hb = self.heartbeat()
        self.assertEqual(hb["state"], "idle")
        self.assertIsNone(hb["current"])
        self.assertEqual(hb["worker"], "test-rev")
        self.assertEqual(hb["version"], 1)
        self.assertLessEqual(abs(hb["updated_unix"] - time.time()), 5)
        self.assertFalse((self.root / "worker.json.tmp").exists())

    def test_heartbeat_busy_while_working(self):
        os.environ["PDF_WORKER_VERSION"] = "test-rev"
        beat = pdf_runner.Heartbeat(self.root)
        beat.start("a1")
        self.assertEqual(self.heartbeat()["state"], "busy")
        self.assertEqual(self.heartbeat()["current"], "a1")
        beat.start("a2")
        beat.finish("a1")
        self.assertEqual(self.heartbeat()["current"], "a2")
        beat.idle()
        self.assertEqual(self.heartbeat()["state"], "idle")
        self.assertIsNone(self.heartbeat()["current"])

    def test_paths_are_used_as_they_are_without_a_map(self):
        self.assertEqual(pdf_runner.to_local("/biblioteca/a b/c.pdf", []), Path("/biblioteca/a b/c.pdf"))

    def test_path_map_longest_prefix(self):
        pairs = sorted([("/biblioteca", "/mnt/b"), ("/biblioteca/x", "/mnt/x"), ("/config", "/srv/c")],
                       key=lambda p: -len(p[0]))
        self.assertEqual(pdf_runner.to_local("/biblioteca/x/y.pdf", pairs), Path("/mnt/x/y.pdf"))
        self.assertEqual(pdf_runner.to_local("/biblioteca/xy.pdf", pairs), Path("/mnt/b/xy.pdf"))
        with self.assertRaises(ValueError):
            pdf_runner.to_local("/bibliotecas/a.pdf", pairs)

    def test_heavy_slot_takes_a_free_slot_of_the_pool(self):
        slots = Path(self.tmp.name) / "slots"
        slots.mkdir()
        held = (slots / "1.lock").open("a+")          # slot 1 is busy (another job)
        fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
        old = pdf_runner.SLOTS_DIR
        pdf_runner.SLOTS_DIR = slots
        try:
            with pdf_runner.heavy_slot("test step"):
                self.assertIn("test step", (slots / "2.owner").read_text())
                with (slots / "2.lock").open("a+") as probe, self.assertRaises(OSError):
                    fcntl.flock(probe, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with (slots / "2.lock").open("a+") as probe:
                fcntl.flock(probe, fcntl.LOCK_EX | fcntl.LOCK_NB)   # released again
        finally:
            pdf_runner.SLOTS_DIR = old
            held.close()

    def watcher(self, **extra):
        env = dict(self.env, PDF_RUNNER_FAKE="done", STUMP_PDF_QUEUE=str(self.root),
                   STUMP_PATH_MAP=f"{self.root}={self.root},/biblioteca={self.books}",
                   PDF_RUNNER=str(HERE / "pdf_runner.py"), PDF_WORKER_PYTHON=sys.executable, **extra)
        return subprocess.Popen(["bash", str(HERE / "pdf_worker.sh")], env=env, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, text=True, start_new_session=True)

    def stop(self, proc):
        os.killpg(proc.pid, signal.SIGTERM)
        return proc.communicate(timeout=10)[0]

    def wait_for(self, path, seconds):
        deadline = time.time() + seconds
        while not path.exists() and time.time() < deadline:
            time.sleep(0.1)
        return path.exists()

    def queue_like_stump(self, rid):
        """Stump's write_request: <id>.json.tmp written, then renamed to <id>.json."""
        st = self.pdf.stat()
        tmp = self.root / "queue" / f"{rid}.json.tmp"
        tmp.write_text(json.dumps({"version": 1, "id": rid, "steps": ["ocr"],
                                   "path": "/biblioteca/biblioteca/A_Author/Scan.pdf",
                                   "library_path": "/biblioteca/biblioteca", "bytes": st.st_size,
                                   "mtime": int(st.st_mtime), "language": None,
                                   "created_at": "2026-11-02T00:00:00Z"}))
        os.replace(tmp, self.root / "queue" / f"{rid}.json")

    @unittest.skipUnless(shutil.which("ionice") and shutil.which("bash"), "needs bash and ionice")
    def test_watcher_polls_drains_and_beats(self):
        proc = self.watcher(PDF_WORKER_POLL="1", PDF_WORKER_HEARTBEAT="10")
        try:
            self.assertTrue(self.wait_for(self.root / "worker.json", 20))
            self.assertEqual(self.heartbeat()["state"], "idle")
            self.request()
            self.assertTrue(self.wait_for(self.root / "done/t1.json", 20))
            self.assertEqual(self.result()["status"], "done")
        finally:
            out = self.stop(proc)
        self.assertIn("watching", out)
        self.assertIn("poll every 1s", out)

    @unittest.skipUnless(shutil.which("ionice") and shutil.which("inotifywait"), "needs inotifywait")
    def test_watcher_wakes_on_every_request_written_like_stump(self):
        # a long heartbeat: only inotify can wake the watcher within the test's few seconds
        proc = self.watcher(PDF_WORKER_HEARTBEAT="600")
        try:
            self.assertTrue(self.wait_for(self.root / "worker.json", 20))
            for i in range(3):                       # a burst, as Queue PDFs writes it
                self.queue_like_stump(f"b{i}")
            for i in range(3):
                self.assertTrue(self.wait_for(self.root / f"done/b{i}.json", 15), f"b{i} not handled")
            time.sleep(2)                            # the watcher is waiting again
            self.queue_like_stump("late")
            self.assertTrue(self.wait_for(self.root / "done/late.json", 15), "a later request was missed")
        finally:
            out = self.stop(proc)
        self.assertIn("(inotify,", out)


if __name__ == "__main__":
    unittest.main(verbosity=2)
