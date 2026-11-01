#!/usr/bin/env python3
"""Host runner for Stump's PDF post-processing queue: OCR and size reduction of scanned PDFs.

Stump (the `post_processing` job) writes one request per PDF into `<config>/workers/pdf/queue/`.
This runner starts when that folder is not empty (systemd path unit `stump-pdf-runner.path`,
or a cron tick), works through the queue and exits: nothing stays resident. It reuses
book-curator's recipe and code unchanged (research/pdf-size.md, data/acquisition/pdf-size/,
data/acquisition/pdf-ocr/):

  1. classify  map_pdfs.analyse + summarise: born-digital PDFs are skipped (their text is the
               real text); for scans, the text-layer class (ok / no_text / partial / poor_ocr)
               and the OCR mode (--skip-text or --redo-ocr) and language.
  2. OCR       only when the text layer is missing or poor: OCRmyPDF 17.13 + Tesseract (fast
               models), -O 1, spa+eng for Spanish books, eng otherwise, in
               jbarlow83/ocrmypdf-ubuntu:v17.13.0 (--memory 900m --cpus 1, tmpfs, read-only
               input); kept when every page is there and >= 90 % of pages have text
               (ocr_queue.verify).
  3. shrink    'auto200' (make_plan.METHOD): a 12-page excerpt first (measure.py; not worth it
               under 12 %), then the whole file in chunks of <= 80 pages / 80 MB through
               shrink_worker.py in the same container, grafted back onto the original page
               objects, and the decision rule (shrink_queue.RULE / decide): same page count,
               same text on every page, same outline, MuPDF structural SSIM >= 0.90 at Kobo size
               and >= 0.88 at 2x, saving >= 15 % and >= 2 MB.

Every heavy step is one job of book-curator's shared g3 pool (data/acquisition/tools/g3slot:
nice 19, idle I/O, 3 slots for every session together), and waits for >= 3 GB of free memory.
The result goes back to Stump as `done/<id>.json` (+ `done/<id>.pdf` when there is a new file);
Stump swaps it in (the original goes to its quarantine folder) only if the library file did not
change meanwhile. No GPU: OCR (Tesseract) and the image work are CPU-bound.

    pdf_runner.py drain       # what the systemd unit / cron run: the whole queue, then exit
    pdf_runner.py status

Paths: Stump's container paths are mapped to host paths through the container's mounts
(`docker inspect $STUMP_CONTAINER`, default stump-fork), or STUMP_PATH_MAP
("/config=/srv/stump/config,/biblioteca=/mnt/books"). Environment: BOOK_CURATOR (default
/mnt/data2/Code/book-curator), PDF_RUNNER_WORK (default /mnt/nvme/Downloads/stump-pdf-runner),
STUMP_PDF_QUEUE (host path of the queue, if not derived from the mounts).
PDF_RUNNER_FAKE=done|not_worth|skipped|failed answers every request without processing it
(tests of the queue protocol).
"""

from __future__ import annotations

import fcntl
import json
import os
import shutil
import subprocess
import sys
import time
import types
from pathlib import Path

BOOK_CURATOR = Path(os.environ.get("BOOK_CURATOR", "/mnt/data2/Code/book-curator"))
PDF_SIZE = BOOK_CURATOR / "data/acquisition/pdf-size"
PDF_OCR = BOOK_CURATOR / "data/acquisition/pdf-ocr"
G3SLOT = BOOK_CURATOR / "data/acquisition/tools/g3slot"
WORK = Path(os.environ.get("PDF_RUNNER_WORK", "/mnt/nvme/Downloads/stump-pdf-runner"))
CONTAINER = os.environ.get("STUMP_CONTAINER", "stump-fork")
OCR_IMAGE = "jbarlow83/ocrmypdf-ubuntu:v17.13.0"
OCR_MODE_FLAGS = {"skip": ["--skip-text"], "redo": ["--redo-ocr"]}
EXCERPT_MAX_RATIO = 0.88      # make_plan.py --max-excerpt-ratio: the excerpt must save >= 12 %
HERE = Path(__file__).resolve()


def log(msg: str) -> None:
    print(f"{time.strftime('%Y-%m-%d %H:%M:%S')} {msg}", flush=True)


# ------------------------------------------------------------------------------------------
# paths
# ------------------------------------------------------------------------------------------

def path_map() -> list[tuple[str, str]]:
    """(container prefix, host prefix), longest container prefix first."""
    pairs: list[tuple[str, str]] = []
    env = os.environ.get("STUMP_PATH_MAP", "").strip()
    if env:
        for item in env.split(","):
            if "=" in item:
                c, h = item.split("=", 1)
                pairs.append((c.rstrip("/") or "/", h.rstrip("/") or "/"))
    else:
        out = subprocess.run(["docker", "inspect", "-f", "{{json .Mounts}}", CONTAINER],
                             capture_output=True, text=True)
        if out.returncode != 0:
            raise SystemExit(f"docker inspect {CONTAINER} failed: {out.stderr.strip()} "
                             "(set STUMP_PATH_MAP)")
        for m in json.loads(out.stdout):
            pairs.append((m["Destination"].rstrip("/") or "/", m["Source"].rstrip("/") or "/"))
    return sorted(pairs, key=lambda p: -len(p[0]))


def to_host(container_path: str, pairs: list[tuple[str, str]]) -> Path:
    for c, h in pairs:
        if container_path == c or container_path.startswith(c + "/"):
            return Path(h + container_path[len(c):])
    raise ValueError(f"{container_path} is under no mount of {CONTAINER}")


def container_config_dir() -> str:
    out = subprocess.run(["docker", "inspect", "-f", "{{json .Config.Env}}", CONTAINER],
                         capture_output=True, text=True)
    if out.returncode == 0:
        for item in json.loads(out.stdout):
            if item.startswith("STUMP_CONFIG_DIR="):
                return item.split("=", 1)[1]
    return "/config"


def queue_root(pairs: list[tuple[str, str]]) -> Path:
    env = os.environ.get("STUMP_PDF_QUEUE")
    if env:
        return Path(env)
    config = "/config" if os.environ.get("STUMP_PATH_MAP") else container_config_dir()
    return to_host(config.rstrip("/") + "/workers/pdf", pairs)


# ------------------------------------------------------------------------------------------
# book-curator steps
# ------------------------------------------------------------------------------------------

def curator_modules():
    for p in (str(PDF_SIZE), str(PDF_OCR)):
        if p not in sys.path:
            sys.path.insert(0, p)
    import shrink  # noqa: E402
    import shrink_queue  # noqa: E402
    return shrink, shrink_queue


def python() -> str:
    venv = BOOK_CURATOR / ".venv/bin/python"
    return str(venv) if venv.exists() else sys.executable


def slot_step(args: list[str], logf: Path) -> int:
    """A python step of this runner as one job of the shared g3 pool."""
    with logf.open("a") as fh:
        fh.write(f"\n## {time.strftime('%Y-%m-%d %H:%M:%S')} {' '.join(args)}\n")
        fh.flush()
        return subprocess.run([str(G3SLOT), python(), str(HERE), *args], stdout=fh, stderr=fh).returncode


def cmd_classify(src: str, out: str, language: str) -> int:
    """_classify: map_pdfs analyse + summarise for one file (born_digital, cls, mode, language)."""
    os.environ["PDFOCR_LIB_ROOT"] = str(Path(src).parent)
    os.environ.setdefault("PDFOCR_WORDLISTS", str(PDF_OCR / "wordlists"))
    sys.path.insert(0, str(PDF_OCR))
    import map_pdfs  # noqa: E402
    d = map_pdfs.analyse(Path(src))
    row = {"language": language} if language else {}
    res = map_pdfs.summarise(d, {d["path"]: row} if row else {})
    keep = ("cls", "born_digital", "mode", "queue", "language", "pages", "text_coverage",
            "quality_score", "notes", "dpi_median")
    Path(out).write_text(json.dumps({k: res.get(k) for k in keep}))
    return 0


def cmd_split(src: str, workdir: str) -> int:
    _, sq = curator_modules()
    return sq.cmd_split(types.SimpleNamespace(source=src, workdir=workdir))


def cmd_final(src: str, workdir: str, out: str) -> int:
    """_final: graft the chunks (if any) and check the result against the source, as
    shrink_queue.cmd_final does (without the library bookkeeping)."""
    import numpy as np
    import pymupdf
    _, sq = curator_modules()
    src_p, wdir = Path(src), Path(workdir)
    plan = json.loads((wdir / "plan.json").read_text())
    chunks = [(f, l, wdir / f"chunk{k:04d}" / "out.pdf") for k, (f, l) in enumerate(plan["chunks"])]
    if plan.get("whole"):
        merged = chunks[0][2]
    else:
        merged = wdir / "merged.pdf"
        sq.graft(src_p, chunks, merged)
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
    res["metrics"], res["quality"] = sq.quality_metrics(da, db, pick)
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
    sys.path.insert(0, str(PDF_OCR))
    import ocr_queue  # noqa: E402
    Path(res_path).write_text(json.dumps(ocr_queue.verify(Path(src), Path(out))))
    return 0


def ocr(src: Path, wdir: Path, lang: str, mode: str, rid: str) -> tuple[Path | None, dict]:
    """OCRmyPDF in the research's container through the g3 pool; (output, record)."""
    out_dir = wdir / "ocr"
    shutil.rmtree(out_dir, ignore_errors=True)
    (out_dir / "tmp").mkdir(parents=True)
    args = ["ocrmypdf", "--output-type", "pdf", "-O", "1", "--fast-web-view", "999999", "--jobs", "1",
            "-l", lang, "--tesseract-timeout", "900", "--continue-on-soft-render-error",
            *OCR_MODE_FLAGS[mode], "/data/in.pdf", "/work/out.pdf"]
    cmd = [str(G3SLOT), "docker", "run", "--rm", "--read-only", "--log-driver", "none",
           "--name", f"stump-pdf-ocr-{rid[:12]}", "--memory", "900m", "--memory-swap", "900m",
           "--cpus", "1", "--cpu-shares", "64", "--user", f"{os.getuid()}:{os.getgid()}",
           "--tmpfs", "/tmp:rw,size=64m,mode=1777", "-v", f"{src}:/data/in.pdf:ro",
           "-v", f"{out_dir}:/work", "-w", "/work", "-e", "TMPDIR=/work/tmp", "-e", "HOME=/tmp",
           "-e", "OMP_THREAD_LIMIT=1", "--entrypoint", "sh", OCR_IMAGE, "-c",
           "ionice -c3 nice -n 19 " + " ".join(f"'{x}'" for x in args)]
    t0 = time.time()
    p = subprocess.run(cmd, capture_output=True, text=True)
    rec = {"lang": lang, "mode": mode, "rc": p.returncode, "seconds": round(time.time() - t0, 1),
           "log_tail": p.stderr.strip().splitlines()[-4:]}
    out = out_dir / "out.pdf"
    if p.returncode != 0 or not out.exists():
        return None, rec
    vf = wdir / "ocr_verify.json"
    if slot_step(["_verify_ocr", str(src), str(out), str(vf)], wdir / "steps.log") != 0 or not vf.exists():
        rec["verify"] = "failed"
        return None, rec
    v = json.loads(vf.read_text())
    rec["verify"] = {k: v.get(k) for k in ("pages_before", "pages_after", "text_pages_before",
                                            "text_pages_after", "known_word_share", "bytes_before",
                                            "bytes_after")}
    ok = v["pages_before"] == v["pages_after"] and v["text_pages_after"] >= 0.9 * v["pages_after"]
    return (out if ok else None), rec


def excerpt_ratio(src: Path, wdir: Path, params: dict, rid: str) -> float | None:
    """measure.py: 12 pages over 5-95 % of the book through the worker; the size ratio."""
    import pikepdf
    shrink, _ = curator_modules()
    ex_dir = wdir / "excerpt"
    ex_dir.mkdir(parents=True, exist_ok=True)
    ex = ex_dir / "excerpt.pdf"
    with pikepdf.open(src) as pdf:
        n = len(pdf.pages)
    pages = sorted({int(n * (0.05 + 0.9 * i / 11)) for i in range(12)}) if n > 12 else list(range(n))
    shrink.excerpt(src, pages, ex)
    p = dict(params, metric_pages=list(range(0, len(pages), 3)), crops=False)
    if not shrink.wait_for_memory():
        return None
    r = shrink.local_shrink(ex, p, ex_dir / "out", f"stump-pdf-measure-{rid[:12]}")
    shutil.rmtree(ex_dir, ignore_errors=True)
    return r["report"].get("ratio") if r["ok"] else None


def shrink_file(src: Path, wdir: Path, rid: str, measured: bool) -> tuple[str, str, Path | None, dict]:
    """(status, reason, output, record) of the auto200 size step on src."""
    shrink, sq = curator_modules()
    sys.path.insert(0, str(PDF_SIZE))
    import make_plan  # noqa: E402
    params = make_plan.METHOD["auto200"]
    rec: dict = {"method": "auto200"}
    if measured:
        ratio = excerpt_ratio(src, wdir, params, rid)
        rec["excerpt_ratio"] = ratio
        if ratio is None:
            return "failed", "the 12-page excerpt could not be measured", None, rec
        if ratio > EXCERPT_MAX_RATIO:
            return "not_worth", f"a 12-page excerpt saved only {1 - ratio:.0%}", None, rec
    sdir = wdir / "shrink"
    sdir.mkdir(parents=True, exist_ok=True)
    if not shrink.wait_for_memory():
        return "failed", "paused (book-curator PAUSE file)", None, rec
    if slot_step(["_split", str(src), str(sdir)], wdir / "steps.log") != 0:
        return "failed", "split failed", None, rec
    plan = json.loads((sdir / "plan.json").read_text())
    reports = []
    for k, (first, last) in enumerate(plan["chunks"]):
        cdir = sdir / f"chunk{k:04d}"
        if not shrink.wait_for_memory():
            return "failed", "paused (book-curator PAUSE file)", None, rec
        p = dict(params, metric_auto=0, metric_pages=[], crops=False)
        r = shrink.local_shrink(src if plan.get("whole") else cdir / "in.pdf", p, cdir,
                                f"stump-pdf-shrink-{rid[:8]}-{k:04d}")
        if not r["ok"]:
            return "failed", f"chunk {k} p{first}-{last}: rc={r['rc']}", None, rec
        reports.append(r["report"])
    fin = sdir / "final.json"
    if slot_step(["_final", str(src), str(sdir), str(fin)], wdir / "steps.log") != 0 or not fin.exists():
        return "failed", "the final check failed", None, rec
    final = json.loads(fin.read_text())
    st = {"final": {k: v for k, v in final.items() if k not in ("merged", "metrics", "quality")},
          "quality": final.get("quality") or {}}
    status, reason = sq.decide(st)
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
        src = to_host(req["path"], pairs)
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
        # a private copy: the library file may change while we work, and odd characters in
        # library paths (':') break docker's -v
        work_src = wdir / "source.pdf"
        shutil.copyfile(src, work_src)
        cf = wdir / "classify.json"
        if slot_step(["_classify", str(work_src), str(cf), req.get("language") or ""],
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
                    log(f"{rid}: OCR ({c['mode']}, {lang})")
                    ocr_out, rec = ocr(work_src, wdir, lang, c["mode"], rid)
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
            s_status, s_reason, shrunk, rec = shrink_file(source, wdir, rid, measured=ocr_out is None)
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
    handled = 0
    while True:
        todo = sorted((root / "queue").glob("*.json"), key=lambda f: f.stat().st_mtime)
        if not todo:
            break
        f = todo[0]
        running = root / "running" / f.name
        try:
            os.replace(f, running)
        except FileNotFoundError:
            continue
        try:
            req = json.loads(running.read_text())
        except ValueError:
            log(f"unreadable request {f.name}, dropped")
            running.unlink(missing_ok=True)
            continue
        log(f"{req['id']}: {req['path']} {req.get('steps')}")
        try:
            res = process(req, root, pairs)
        except Exception as exc:  # noqa: BLE001
            res = {"id": req["id"], "status": "failed", "reason": f"{type(exc).__name__}: {exc}"[:300]}
        res["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
        write_json(root / "done" / f"{req['id']}.json", res)
        running.unlink(missing_ok=True)
        log(f"{req['id']}: {res['status']} - {res.get('reason', '')}")
        handled += 1
    log(f"queue empty ({handled} handled)")
    return 0


def status() -> int:
    root = queue_root(path_map())
    for d in ("queue", "running", "done", "archive"):
        print(f"{d}: {len(list((root / d).glob('*.json')))}")
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
