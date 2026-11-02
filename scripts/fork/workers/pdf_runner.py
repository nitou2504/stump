#!/usr/bin/env python3
"""The PDF worker of Stump's post-processing: OCR and size reduction of scanned PDFs.

It runs in its own container (docker/Dockerfile.pdf-worker, the `stump-pdf-worker` service next
to Stump in docker compose): Stump writes one request per PDF into `<config>/workers/pdf/queue/`,
the container's watcher (pdf_worker.sh) starts `pdf_runner.py drain` only when that folder has
something in it, and drain works through the queue and exits. Waiting costs a shell and an
inotifywait, a few MB. The recipe is book-curator's (research/pdf-size.md), vendored in
pdfrecipe/ so the image needs no book-curator checkout:

  1. classify  map_pdfs.analyse + summarise: born-digital PDFs are skipped (their text is the
               real text); for scans, the text-layer class (ok / no_text / partial / poor_ocr)
               and the OCR mode (--skip-text or --redo-ocr) and language.
  2. OCR       only when the text layer is missing or poor: OCRmyPDF 17.13 + Tesseract (fast
               models), -O 1, one job, spa+eng for Spanish books, eng otherwise; kept when
               every page is there and >= 90 % of pages have text (ocr_queue.verify).
  3. shrink    'auto200' (make_plan.METHOD): a 12-page excerpt first (not worth it under 12 %),
               then the whole file in chunks of <= 80 pages / 80 MB through shrink_worker.py,
               grafted back onto the original page objects, and the decision rule (RULE /
               decide): same page count, same text on every page, same outline, MuPDF
               structural SSIM >= 0.90 at Kobo size and >= 0.88 at 2x, saving >= 15 % and >= 2 MB.

The container itself is the sandbox (compose sets its CPU and memory limits); every step runs at
nice 19 and idle I/O priority, one PDF at a time, and waits while the host has less than
PDF_WORKER_MIN_FREE_MB (default 1536) of available memory or while `<queue>/PAUSE` exists.
When book-curator's g3 pool folder is mounted (G3_SLOTS_DIR, default /g3-heavy-slots), every
heavy step also takes one of its slots, so the worker shares g3 with the curator's own jobs.

The result goes back to Stump as `done/<id>.json` (+ `done/<id>.pdf` when there is a new file);
Stump swaps it in (the original goes to its quarantine folder) only if the library file did not
change meanwhile. While it works, drain refreshes the worker's heartbeat (`worker.json`) that
Stump shows in the library's post-processing settings. No GPU: OCR (Tesseract) and the image
work are CPU-bound.

    pdf_runner.py drain       # what the watcher runs: the whole queue, then exit
    pdf_runner.py status

The request paths are Stump's container paths; the worker mounts the same folders at the same
paths, so they resolve as they are. STUMP_PATH_MAP ("/config=/srv/stump/config,/biblioteca=
/mnt/books") maps them elsewhere (tests, a run outside the container). Environment:
STUMP_PDF_QUEUE (the queue folder, default /config/workers/pdf), PDF_RUNNER_WORK (work files,
default /work), PDF_WORKER_HEARTBEAT (seconds between heartbeats, default 300).
PDF_RUNNER_FAKE=done|not_worth|skipped|failed answers every request without processing it
(tests of the queue protocol).
"""

from __future__ import annotations

import contextlib
import fcntl
import json
import os
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve()
sys.path.insert(0, str(HERE.parent / "pdfrecipe"))

WORK = Path(os.environ.get("PDF_RUNNER_WORK", "/work"))
OCR_MODE_FLAGS = {"skip": ["--skip-text"], "redo": ["--redo-ocr"]}
HEARTBEAT_FILE = "worker.json"
HEARTBEAT_VERSION = 1
MIN_FREE_MB = int(os.environ.get("PDF_WORKER_MIN_FREE_MB", "1536"))
SLOTS_DIR = Path(os.environ.get("G3_SLOTS_DIR", "/g3-heavy-slots"))
SLOTS = int(os.environ.get("G3_SLOTS", "3"))
STEP_TIMEOUT = 6 * 3600


def log(msg: str) -> None:
    print(f"{time.strftime('%Y-%m-%d %H:%M:%S')} {msg}", flush=True)


# ------------------------------------------------------------------------------------------
# paths
# ------------------------------------------------------------------------------------------

def path_map() -> list[tuple[str, str]]:
    """(container prefix, local prefix), longest prefix first; empty = paths are used as they are."""
    pairs: list[tuple[str, str]] = []
    for item in os.environ.get("STUMP_PATH_MAP", "").split(","):
        if "=" in item:
            c, h = item.split("=", 1)
            pairs.append((c.strip().rstrip("/") or "/", h.strip().rstrip("/") or "/"))
    return sorted(pairs, key=lambda p: -len(p[0]))


def to_local(container_path: str, pairs: list[tuple[str, str]]) -> Path:
    if not pairs:
        return Path(container_path)
    for c, h in pairs:
        if container_path == c or container_path.startswith(c + "/"):
            return Path(h + container_path[len(c):])
    raise ValueError(f"{container_path} is under no mount of the worker")


def queue_root(pairs: list[tuple[str, str]]) -> Path:
    return to_local(os.environ.get("STUMP_PDF_QUEUE", "/config/workers/pdf"), pairs)


# ------------------------------------------------------------------------------------------
# heartbeat (read by Stump: core/src/postproc/pdf_queue.rs, worker_status)
# ------------------------------------------------------------------------------------------

def heartbeat_interval() -> int:
    try:
        return max(10, int(os.environ.get("PDF_WORKER_HEARTBEAT", "300")))
    except ValueError:
        return 300


def write_heartbeat(root: Path, state: str, current: str | None = None) -> None:
    now = time.time()
    data = {"version": HEARTBEAT_VERSION, "worker": os.environ.get("PDF_WORKER_VERSION", "dev"),
            "state": state, "current": current, "pid": os.getpid(),
            "started_at": os.environ.get("PDF_WORKER_STARTED_AT"),
            "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now)),
            "updated_unix": int(now), "interval": heartbeat_interval()}
    try:
        write_json(root / HEARTBEAT_FILE, data)
    except OSError as exc:
        log(f"could not write the heartbeat: {exc}")


class Heartbeat:
    """Refreshes worker.json as 'busy' every interval while drain works on requests."""

    def __init__(self, root: Path):
        self.root, self.current = root, []
        self.lock = threading.Lock()
        self.stop = threading.Event()
        self.thread = threading.Thread(target=self.run, daemon=True)

    def write(self) -> None:
        with self.lock:
            write_heartbeat(self.root, "busy" if self.current else "idle",
                            ",".join(self.current) or None)

    def run(self) -> None:
        while not self.stop.wait(heartbeat_interval()):
            self.write()

    def start(self, rid: str) -> None:
        with self.lock:
            self.current.append(rid)
            if not self.thread.is_alive() and not self.stop.is_set():
                self.thread.start()
        self.write()

    def finish(self, rid: str) -> None:
        with self.lock:
            if rid in self.current:
                self.current.remove(rid)
        self.write()

    def idle(self) -> None:
        self.stop.set()
        if self.thread.is_alive():
            self.thread.join(timeout=10)
        with self.lock:
            self.current = []
        self.write()


# ------------------------------------------------------------------------------------------
# heavy steps
# ------------------------------------------------------------------------------------------

def host_mem_available_mb() -> int:
    """MemAvailable of the host (a container's /proc/meminfo shows the host's)."""
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) // 1024
    except OSError:
        pass
    return 1 << 30


def wait_for_room(root: Path) -> bool:
    """Before a heavy step: False while `<queue>/PAUSE` exists (the request goes back to the
    queue), otherwise wait until the host has MIN_FREE_MB of memory available."""
    while True:
        if (root / "PAUSE").exists():
            return False
        mem = host_mem_available_mb()
        if mem >= MIN_FREE_MB:
            return True
        log(f"waiting: the host has {mem} MB available < {MIN_FREE_MB} MB")
        time.sleep(60)


@contextlib.contextmanager
def heavy_slot(what: str):
    """One slot of book-curator's g3 pool (data/acquisition/tools/g3slot: flock files
    <dir>/<n>.lock) when that folder is mounted; otherwise nothing."""
    if not SLOTS_DIR.is_dir() or not os.access(SLOTS_DIR, os.W_OK):
        yield
        return
    fh = None
    while fh is None:
        for i in range(1, SLOTS + 1):
            f = (SLOTS_DIR / f"{i}.lock").open("a+")
            try:
                fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                f.close()
                continue
            fh = f
            with contextlib.suppress(OSError):
                (SLOTS_DIR / f"{i}.owner").write_text(
                    f"stump-pdf-worker pid {os.getpid()} since {time.strftime('%Y-%m-%dT%H:%M:%S%z')}: {what}"[:300])
            break
        if fh is None:
            time.sleep(10)
    try:
        yield
    finally:
        fh.close()


def step(args: list[str], logf: Path) -> int:
    """A python step of this runner in a fresh process (its memory goes back when it exits)."""
    with logf.open("a") as fh:
        fh.write(f"\n## {time.strftime('%Y-%m-%d %H:%M:%S')} {' '.join(args)}\n")
        fh.flush()
        with heavy_slot(args[0]):
            try:
                return subprocess.run([sys.executable, str(HERE), *args], stdout=fh, stderr=fh,
                                      timeout=STEP_TIMEOUT).returncode
            except subprocess.TimeoutExpired:
                return -9


def cmd_classify(src: str, out: str, language: str) -> int:
    """_classify: map_pdfs analyse + summarise for one file (born_digital, cls, mode, language)."""
    os.environ["PDFOCR_LIB_ROOT"] = str(Path(src).parent)
    import map_pdfs  # noqa: E402
    d = map_pdfs.analyse(Path(src))
    row = {"language": language} if language else {}
    res = map_pdfs.summarise(d, {d["path"]: row} if row else {})
    keep = ("cls", "born_digital", "mode", "queue", "language", "pages", "text_coverage",
            "quality_score", "notes", "dpi_median")
    Path(out).write_text(json.dumps({k: res.get(k) for k in keep}))
    return 0


def cmd_split(src: str, workdir: str) -> int:
    import recipe  # noqa: E402
    return recipe.split(src, workdir)


def cmd_final(src: str, workdir: str, out: str) -> int:
    """_final: graft the chunks (if any) and check the result against the source, as
    shrink_queue.cmd_final does (without the library bookkeeping)."""
    import numpy as np
    import pymupdf
    import recipe  # noqa: E402
    src_p, wdir = Path(src), Path(workdir)
    plan = json.loads((wdir / "plan.json").read_text())
    chunks = [(f, l, wdir / f"chunk{k:04d}" / "out.pdf") for k, (f, l) in enumerate(plan["chunks"])]
    if plan.get("whole"):
        merged = chunks[0][2]
    else:
        merged = wdir / "merged.pdf"
        recipe.graft(src_p, chunks, merged)
    res: dict = {"merged": str(merged), "bytes_in": src_p.stat().st_size, "bytes_out": merged.stat().st_size}
    da, db = pymupdf.open(src_p), pymupdf.open(merged)
    res["pages_in"], res["pages_out"] = da.page_count, db.page_count
    same, diff = 0, []
    if da.page_count == db.page_count:
        for i in range(da.page_count):
            if da[i].get_text() == db[i].get_text():
                same += 1
            elif len(diff) < 20:
                diff.append(i + 1)
    res["text_pages_identical"], res["text_pages_differ"] = same, diff
    res["toc_same"] = da.get_toc(simple=True) == db.get_toc(simple=True)
    ma, mb = dict(da.metadata or {}), dict(db.metadata or {})
    res["metadata_same"] = {k: v for k, v in ma.items() if k != "format"} == \
        {k: v for k, v in mb.items() if k != "format"}
    n = min(da.page_count, db.page_count)
    pick = {int(n * (i + 0.5) / 6) for i in range(6)}
    for (first, _l), path in zip(plan["chunks"], [c[2] for c in chunks]):
        rep_f = path.parent / "report.json"
        if rep_f.exists():
            for p in (json.loads(rep_f.read_text()).get("changed_pages_top") or [])[:2]:
                pick.add(first - 1 + p)
    res["metrics"], res["quality"] = recipe.quality_metrics(da, db, pick)
    worst = 0.0
    for i in sorted({0, n // 4, n // 2, (3 * n) // 4, n - 1}):
        if i < 0 or i >= n:
            continue
        x = np.frombuffer(da[i].get_pixmap(dpi=40, colorspace=pymupdf.csGRAY).samples, dtype=np.uint8)
        y = np.frombuffer(db[i].get_pixmap(dpi=40, colorspace=pymupdf.csGRAY).samples, dtype=np.uint8)
        if x.shape != y.shape:
            worst = 255.0
            continue
        worst = max(worst, float(np.abs(x.astype(np.int16) - y.astype(np.int16)).mean()))
    res["lowres_mean_abs_diff_max"] = round(worst, 2)
    Path(out).write_text(json.dumps(res, indent=1))
    return 0


def cmd_verify_ocr(src: str, out: str, res_path: str) -> int:
    import recipe  # noqa: E402
    Path(res_path).write_text(json.dumps(recipe.verify(Path(src), Path(out))))
    return 0


def ocr(src: Path, wdir: Path, lang: str, mode: str) -> tuple[Path | None, dict]:
    """OCRmyPDF (this image's) on src, single-threaded; (output, record)."""
    out_dir = wdir / "ocr"
    shutil.rmtree(out_dir, ignore_errors=True)
    (out_dir / "tmp").mkdir(parents=True)
    out = out_dir / "out.pdf"
    args = ["ocrmypdf", "--output-type", "pdf", "-O", "1", "--fast-web-view", "999999", "--jobs", "1",
            "-l", lang, "--tesseract-timeout", "900", "--continue-on-soft-render-error",
            *OCR_MODE_FLAGS[mode], str(src), str(out)]
    env = dict(os.environ, TMPDIR=str(out_dir / "tmp"), OMP_THREAD_LIMIT="1")
    t0 = time.time()
    with heavy_slot("ocrmypdf"):
        try:
            p = subprocess.run(args, capture_output=True, text=True, env=env, cwd=out_dir, timeout=STEP_TIMEOUT)
            rc, err = p.returncode, p.stderr
        except subprocess.TimeoutExpired:
            rc, err = -9, "timed out"
    shutil.rmtree(out_dir / "tmp", ignore_errors=True)
    rec = {"lang": lang, "mode": mode, "rc": rc, "seconds": round(time.time() - t0, 1),
           "log_tail": err.strip().splitlines()[-4:]}
    if rc != 0 or not out.exists():
        return None, rec
    vf = wdir / "ocr_verify.json"
    if step(["_verify_ocr", str(src), str(out), str(vf)], wdir / "steps.log") != 0 or not vf.exists():
        rec["verify"] = "failed"
        return None, rec
    v = json.loads(vf.read_text())
    rec["verify"] = {k: v.get(k) for k in ("pages_before", "pages_after", "text_pages_before",
                                            "text_pages_after", "known_word_share", "bytes_before",
                                            "bytes_after")}
    ok = v["pages_before"] == v["pages_after"] and v["text_pages_after"] >= 0.9 * v["pages_after"]
    return (out if ok else None), rec


def excerpt_ratio(src: Path, wdir: Path, params: dict, root: Path) -> float | None:
    """measure.py: 12 pages over 5-95 % of the book through shrink_worker.py; the size ratio."""
    import pikepdf
    import recipe  # noqa: E402
    ex_dir = wdir / "excerpt"
    ex_dir.mkdir(parents=True, exist_ok=True)
    ex = ex_dir / "excerpt.pdf"
    with pikepdf.open(src) as pdf:
        n = len(pdf.pages)
    pages = sorted({int(n * (0.05 + 0.9 * i / 11)) for i in range(12)}) if n > 12 else list(range(n))
    recipe.excerpt(src, pages, ex)
    p = dict(params, metric_pages=list(range(0, len(pages), 3)), crops=False)
    if not wait_for_room(root):
        return None
    with heavy_slot("excerpt"):
        r = recipe.local_shrink(ex, p, ex_dir / "out")
    shutil.rmtree(ex_dir, ignore_errors=True)
    return r["report"].get("ratio") if r["ok"] else None


class Paused(Exception):
    pass


def shrink_file(src: Path, wdir: Path, root: Path, measured: bool) -> tuple[str, str, Path | None, dict]:
    """(status, reason, output, record) of the auto200 size step on src."""
    import recipe  # noqa: E402
    params = recipe.METHOD["auto200"]
    rec: dict = {"method": "auto200"}
    if measured:
        ratio = excerpt_ratio(src, wdir, params, root)
        rec["excerpt_ratio"] = ratio
        if ratio is None:
            if (root / "PAUSE").exists():
                raise Paused
            return "failed", "the 12-page excerpt could not be measured", None, rec
        if ratio > recipe.EXCERPT_MAX_RATIO:
            return "not_worth", f"a 12-page excerpt saved only {1 - ratio:.0%}", None, rec
    sdir = wdir / "shrink"
    sdir.mkdir(parents=True, exist_ok=True)
    if not wait_for_room(root):
        raise Paused
    if step(["_split", str(src), str(sdir)], wdir / "steps.log") != 0:
        return "failed", "split failed", None, rec
    plan = json.loads((sdir / "plan.json").read_text())
    for k, (first, last) in enumerate(plan["chunks"]):
        cdir = sdir / f"chunk{k:04d}"
        if not wait_for_room(root):
            raise Paused
        p = dict(params, metric_auto=0, metric_pages=[], crops=False)
        with heavy_slot(f"shrink chunk {k}"):
            r = recipe.local_shrink(src if plan.get("whole") else cdir / "in.pdf", p, cdir)
        if not r["ok"]:
            return "failed", f"chunk {k} p{first}-{last}: rc={r['rc']}", None, rec
    fin = sdir / "final.json"
    if step(["_final", str(src), str(sdir), str(fin)], wdir / "steps.log") != 0 or not fin.exists():
        return "failed", "the final check failed", None, rec
    final = json.loads(fin.read_text())
    st = {"final": {k: v for k, v in final.items() if k not in ("merged", "metrics", "quality")},
          "quality": final.get("quality") or {}}
    status, reason = recipe.decide(st)
    rec.update(final=st["final"], quality={k: st["quality"].get(k) for k in (
        "ssim_struct_min_kobo", "ssim_struct_min_zoom2", "ssim_ink_min_kobo", "psnr_min_kobo",
        "pages_measured")}, chunks=len(plan["chunks"]))
    return status, reason, (Path(final["merged"]) if status == "done" else None), rec


# ------------------------------------------------------------------------------------------
# queue
# ------------------------------------------------------------------------------------------

def write_json(path: Path, data: dict) -> None:
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps(data, ensure_ascii=False, indent=1))
    os.replace(tmp, path)


def mb(n: int) -> str:
    return f"{n / 1e6:.1f} MB"


def process(req: dict, root: Path, pairs: list[tuple[str, str]]) -> dict:
    rid = req["id"]
    res: dict = {"id": rid, "status": "failed", "reason": "", "details": {"steps": req.get("steps")}}
    try:
        src = to_local(req["path"], pairs)
    except ValueError as exc:
        res["reason"] = str(exc)
        return res
    if not src.exists():
        res["reason"] = "the file is not in the library any more"
        return res
    st = src.stat()
    res["source_bytes"], res["source_mtime"] = st.st_size, int(st.st_mtime)
    if st.st_size != req["bytes"] or int(st.st_mtime) != req["mtime"]:
        res["reason"] = "the file changed after it was queued"
        return res
    fake = os.environ.get("PDF_RUNNER_FAKE")
    if fake:
        res.update(status=fake, reason=f"fake run ({fake})", bytes_in=st.st_size)
        if fake == "done":
            shutil.copyfile(src, root / "done" / f".{rid}.pdf.part")
            os.replace(root / "done" / f".{rid}.pdf.part", root / "done" / f"{rid}.pdf")
            res["bytes_out"] = st.st_size
        return res

    wdir = WORK / rid
    shutil.rmtree(wdir, ignore_errors=True)
    wdir.mkdir(parents=True)
    try:
        # a private copy: the library file may change while we work (the library is read-only here)
        work_src = wdir / "source.pdf"
        shutil.copyfile(src, work_src)
        if not wait_for_room(root):
            raise Paused
        cf = wdir / "classify.json"
        if step(["_classify", str(work_src), str(cf), req.get("language") or ""],
                wdir / "steps.log") != 0 or not cf.exists():
            res["reason"] = "could not read the PDF"
            return res
        c = json.loads(cf.read_text())
        res["details"]["classify"] = c
        if c.get("cls") == "broken":
            res["reason"] = f"unreadable PDF ({c.get('notes', '')[:120]})"
            return res
        if c.get("born_digital") == "yes":
            res.update(status="skipped", reason="born-digital PDF: nothing to OCR or shrink")
            return res
        steps = set(req.get("steps") or [])
        ocr_out, notes = None, []
        if "ocr" in steps:
            if c.get("cls") in ("no_text", "partial", "poor_ocr"):
                if c.get("queue") != "yes":
                    notes.append(f"OCR not run ({c.get('queue')})")
                elif c.get("mode") not in OCR_MODE_FLAGS:
                    notes.append(f"OCR not run (mode {c.get('mode')}: needs book-curator's strip step)")
                else:
                    lang = "spa+eng" if str(c.get("language", "")).startswith("es") else "eng"
                    if not wait_for_room(root):
                        raise Paused
                    log(f"{rid}: OCR ({c['mode']}, {lang})")
                    ocr_out, rec = ocr(work_src, wdir, lang, c["mode"])
                    res["details"]["ocr"] = rec
                    if ocr_out is None:
                        res["reason"] = "OCR failed its check"
                        return res
            else:
                notes.append("the text layer is fine")
        shrunk, s_status, s_reason = None, "", ""
        if "shrink" in steps:
            source = ocr_out or work_src
            log(f"{rid}: size step")
            s_status, s_reason, shrunk, rec = shrink_file(source, wdir, root, measured=ocr_out is None)
            res["details"]["shrink"] = rec
            if s_status == "failed" and ocr_out is None:
                res["reason"] = f"size step: {s_reason}"
                return res
        deliver = shrunk or ocr_out
        if deliver is None:
            reason = "; ".join([*notes, *([s_reason] if s_reason else [])]) or "nothing to do"
            res.update(status="not_worth" if s_status == "not_worth" else "skipped", reason=reason)
            return res
        out = root / "done" / f"{rid}.pdf"
        part = root / "done" / f".{rid}.pdf.part"
        shutil.copyfile(deliver, part)
        os.replace(part, out)
        size = out.stat().st_size
        parts = []
        if ocr_out is not None:
            parts.append("new OCR layer")
        if shrunk is not None:
            parts.append(f"size step: {s_reason}")
        elif s_reason:
            parts.append(f"size step not used ({s_reason})")
        res.update(status="done", reason=f"{'; '.join(parts)} ({mb(st.st_size)} -> {mb(size)})",
                   bytes_in=st.st_size, bytes_out=size)
        return res
    finally:
        shutil.rmtree(wdir, ignore_errors=True)


def take(root: Path) -> tuple[Path, dict] | None:
    """The oldest queued request, moved to running/ (an atomic rename: one taker wins)."""
    while True:
        if (root / "PAUSE").exists():
            log("paused (PAUSE file in the queue folder)")
            return None
        todo = sorted((root / "queue").glob("*.json"), key=lambda f: f.stat().st_mtime)
        if not todo:
            return None
        f = todo[0]
        running = root / "running" / f.name
        try:
            os.replace(f, running)
        except FileNotFoundError:
            continue
        try:
            return running, json.loads(running.read_text())
        except ValueError:
            log(f"unreadable request {f.name}, dropped")
            running.unlink(missing_ok=True)


def work(root: Path, pairs: list[tuple[str, str]], beat: Heartbeat, handled: list[int]) -> None:
    """Take and process requests until the queue is empty or paused."""
    while (taken := take(root)) is not None:
        running, req = taken
        log(f"{req['id']}: {req['path']} {req.get('steps')}")
        beat.start(req["id"])
        try:
            res = process(req, root, pairs)
        except Paused:
            os.replace(running, root / "queue" / running.name)
            log(f"{req['id']}: paused, back in the queue")
            return
        except Exception as exc:  # noqa: BLE001
            res = {"id": req["id"], "status": "failed", "reason": f"{type(exc).__name__}: {exc}"[:300]}
        finally:
            beat.finish(req["id"])
        res["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
        write_json(root / "done" / f"{req['id']}.json", res)
        running.unlink(missing_ok=True)
        log(f"{req['id']}: {res['status']} - {res.get('reason', '')}")
        handled.append(1)


def drain() -> int:
    pairs = path_map()
    root = queue_root(pairs)
    for d in ("queue", "running", "done", "archive"):
        (root / d).mkdir(parents=True, exist_ok=True)
    lockf = (root / "runner.lock").open("a+")
    try:
        fcntl.flock(lockf, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        log("another runner is active")
        return 0
    # requests left in running/ by a runner that died go back to the queue
    for f in (root / "running").glob("*.json"):
        os.replace(f, root / "queue" / f.name)
    try:
        jobs = max(1, int(os.environ.get("PDF_WORKER_JOBS", "1")))
    except ValueError:
        jobs = 1
    beat = Heartbeat(root)
    handled: list[int] = []
    try:
        if jobs == 1:
            work(root, pairs, beat, handled)
        else:
            threads = [threading.Thread(target=work, args=(root, pairs, beat, handled)) for _ in range(jobs)]
            for t in threads:
                t.start()
            for t in threads:
                t.join()
    finally:
        beat.idle()
    log(f"queue empty ({len(handled)} handled)")
    return 0


def status() -> int:
    root = queue_root(path_map())
    for d in ("queue", "running", "done", "archive"):
        print(f"{d}: {len(list((root / d).glob('*.json')))}")
    hb = root / HEARTBEAT_FILE
    print(f"heartbeat: {hb.read_text().strip() if hb.exists() else 'none'}")
    return 0


def main(argv: list[str]) -> int:
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    cmd, args = argv[0], argv[1:]
    if cmd == "drain":
        return drain()
    if cmd == "status":
        return status()
    if cmd == "heartbeat":
        write_heartbeat(queue_root(path_map()), args[0] if args else "idle")
        return 0
    if cmd == "_classify":
        return cmd_classify(args[0], args[1], args[2] if len(args) > 2 else "")
    if cmd == "_split":
        return cmd_split(args[0], args[1])
    if cmd == "_final":
        return cmd_final(args[0], args[1], args[2])
    if cmd == "_verify_ocr":
        return cmd_verify_ocr(args[0], args[1], args[2])
    print(f"unknown command {cmd}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
