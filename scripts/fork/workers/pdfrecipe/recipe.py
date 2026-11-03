"""Stump's recipe for scanned PDFs: the size step and its checks, as the PDF worker runs them.

  CHUNK_PAGES, CHUNK_MB, RULE, plan_chunks, split, graft, quality_metrics, decide: the size step
      ('auto200', METHOD below) over chunks of the file and the rule that decides whether the
      smaller file replaces the original;
  excerpt, local_shrink: the 12-page excerpt measured first, and shrink_worker.py run as a child
      process (the worker container is the sandbox, with its own memory and CPU limits);
  words, verify: the OCR checks, with the wordfreq 3.1 lists in wordlists/ (Zipf >= 1.75).

map_pdfs.py (the text-layer classification) and shrink_worker.py sit next to this file. The
numbers and rules were measured on the library's scans with OCRmyPDF 17.13 and Tesseract's fast
models in the worker's base image: change them only after measuring again.
"""

from __future__ import annotations

import gzip
import json
import os
import random
import re
import resource
import shutil
import subprocess
import sys
import time
import unicodedata
from pathlib import Path

HERE = Path(__file__).resolve().parent

# shrink_queue.py
CHUNK_PAGES = 80
CHUNK_MB = 80
# decision rule (see research/pdf-size.md): keep the reduced copy only if all hold
RULE = {"min_saving_share": 0.15, "min_saving_mb": 2.0,
        # gates on the structural SSIM (both MuPDF renders blurred 3x3 at Kobo size, 5x5 at 2x): plain SSIM
        # also drops to 0.84 / 0.47 when resampling smooths a halftone screen away (looked at: Arostegui
        # photos; structural 0.98); a near-black-and-white page made 1-bit scores 0.93 / 0.92 (strokes a
        # touch heavier, looked at: Griffiths); a third of a page blanked out scores 0.65-0.84 and an
        # inverted image <= 0.04
        "min_ssim_struct_kobo": 0.90, "min_ssim_struct_zoom2": 0.88}

# make_plan.py
METHOD = {
    # adaptive: images over 240 dpi -> 200 dpi; page images (>= half a page) re-encoded as JPEG q75 when
    # that saves >= 15% of the image (lossless and JPX ones included); colour images with no colour at all
    # -> grey; then OCRmyPDF's lossless optimizer (1-bit images -> JBIG2 generic, Flate over JPEGs)
    "auto200": {"target_dpi": 200, "slack": 0.2, "jpeg_q": 75, "requality": True, "flate_to_jpeg": True,
                "gray": True, "jpx": True, "reencode_min_cover": 0.5, "bilevel_auto": True, "pack_levels": True,
                "min_gain": 0.15, "optimize": "2nj"},
}
# make_plan.py --max-excerpt-ratio: a 12-page excerpt must save >= 12 %
EXCERPT_MAX_RATIO = 0.88


# ------------------------------------------------------------------------------------------
# shrink_queue.py
# ------------------------------------------------------------------------------------------

def plan_chunks(pages: int, size: int, max_pages: int, max_bytes: int) -> list[tuple[int, int]]:
    per_page = size / max(pages, 1)
    n = max(1, min(max_pages, int(max_bytes / max(per_page, 1))))
    if pages <= n:
        return [(1, pages)]
    count = -(-pages // n)
    step = -(-pages // count)
    return [(a, min(a + step - 1, pages)) for a in range(1, pages + 1, step)]


def split(source: str | Path, workdir: str | Path) -> int:
    """shrink_queue.cmd_split: write chunkNNNN/in.pdf files and plan.json."""
    import pikepdf
    src, wdir = Path(source), Path(workdir)
    with pikepdf.open(src) as pdf:
        pages = len(pdf.pages)
        plan = plan_chunks(pages, src.stat().st_size, CHUNK_PAGES, CHUNK_MB * 1024 * 1024)
        if len(plan) == 1:                # small file: processed whole, straight from the source
            plan = []
        for k, (first, last) in enumerate(plan):
            cdir = wdir / f"chunk{k:04d}"
            dst = cdir / "in.pdf"
            if dst.exists():
                continue
            cdir.mkdir(parents=True, exist_ok=True)
            out = pikepdf.new()
            for i in range(first - 1, last):
                out.pages.append(pdf.pages[i])
            for page in out.pages:         # links to pages outside the chunk; the original's are kept
                for key in ("/Annots", "/B", "/StructParents"):
                    if key in page.obj:
                        del page.obj[key]
            out.save(dst.with_suffix(".tmp"))
            os.replace(dst.with_suffix(".tmp"), dst)
    (wdir / "plan.json").write_text(json.dumps({"pages": pages, "chunks": plan or [[1, pages]],
                                                "whole": not plan}))
    return 0


def graft(src: Path, chunks: list[tuple[int, int, Path]], dst: Path) -> None:
    import pikepdf
    tmp = dst.with_name(dst.name + ".tmp")
    with pikepdf.open(src) as pdf:
        for first, last, path in chunks:
            with pikepdf.open(path) as ch:
                if len(ch.pages) != last - first + 1:
                    raise RuntimeError(f"{path}: {len(ch.pages)} pages, expected {last - first + 1}")
                for k, i in enumerate(range(first - 1, last)):
                    op, cp = pdf.pages[i].obj, ch.pages[k].obj
                    for key in ("/Contents", "/Resources"):
                        if key not in cp:          # a blank page has no content stream: leave it as it is
                            continue
                        obj = cp[key]
                        if not obj.is_indirect:
                            obj = ch.make_indirect(obj)
                        op[key] = pdf.copy_foreign(obj)
        pdf.remove_unreferenced_resources()
        pdf.save(tmp, object_stream_mode=pikepdf.ObjectStreamMode.generate, compress_streams=True)
    os.replace(tmp, dst)


def quality_metrics(da, db, pick) -> tuple[list, dict]:
    """MuPDF renders of the picked pages (0-based) of both documents, grey, fit into 1072x1448 and at 2x:
    ink-SSIM, PSNR and the structural SSIM the rule uses (both renders blurred 3x3 / 5x5)."""
    import numpy as np
    import pymupdf
    sys.path.insert(0, str(HERE))
    import shrink_worker as SW
    n = min(da.page_count, db.page_count)
    metrics = []
    for i in sorted(x for x in pick if 0 <= x < n):
        pa, pb = da[i], db[i]
        s = min(1072 / pa.rect.width, 1448 / pa.rect.height)
        rec = {"page": i + 1}
        for tag, sc in (("kobo", s), ("zoom2", 2 * s)):
            xa = pa.get_pixmap(matrix=pymupdf.Matrix(sc, sc), colorspace=pymupdf.csGRAY)
            xb = pb.get_pixmap(matrix=pymupdf.Matrix(sc, sc), colorspace=pymupdf.csGRAY)
            ra = np.frombuffer(xa.samples, np.uint8).reshape(xa.height, xa.width)
            rb = np.frombuffer(xb.samples, np.uint8).reshape(xb.height, xb.width)
            if ra.shape != rb.shape:
                rec[tag] = {"shape": [list(ra.shape), list(rb.shape)], "ssim_struct": 0.0}
                continue
            _g, ink = SW.ssim(ra, rb)
            rec[tag] = {"ssim_ink": round(ink, 4), "psnr": SW.psnr(ra, rb),
                        "ssim_struct": round(SW.ssim_structure(ra, rb, 3 if tag == "kobo" else 5), 4)}
        metrics.append(rec)
        pymupdf.TOOLS.store_shrink(100)
    get = lambda tag, key: [m[tag][key] for m in metrics if key in m.get(tag, {})]  # noqa: E731
    quality = {"ssim_struct_min_kobo": min(get("kobo", "ssim_struct"), default=None),
               "ssim_struct_min_zoom2": min(get("zoom2", "ssim_struct"), default=None),
               "ssim_ink_min_kobo": min(get("kobo", "ssim_ink"), default=None),
               "ssim_ink_min_zoom2": min(get("zoom2", "ssim_ink"), default=None),
               "psnr_min_kobo": min(get("kobo", "psnr"), default=None),
               "psnr_min_zoom2": min(get("zoom2", "psnr"), default=None),
               "pages_measured": len(metrics), "renderer": "MuPDF " + pymupdf.mupdf_version,
               "worst_pages": sorted(({"page": m["page"], "kobo": m["kobo"].get("ssim_ink"),
                                       "zoom2": m["zoom2"].get("ssim_ink"),
                                       "kobo_struct": m["kobo"].get("ssim_struct"),
                                       "zoom2_struct": m["zoom2"].get("ssim_struct")} for m in metrics
                                      if "ssim_struct" in m.get("kobo", {}) and "ssim_struct" in m.get("zoom2", {})),
                                     key=lambda r: r["kobo_struct"])[:5]}
    return metrics, quality


def decide(st: dict) -> tuple[str, str]:
    f = st.get("final") or {}
    if f.get("pages_in") != f.get("pages_out"):
        return "failed", "page count changed"
    if f.get("text_pages_identical") != f.get("pages_in"):
        return "failed", f"text differs on pages {f.get('text_pages_differ')}"
    if not f.get("toc_same"):
        return "failed", "outline differs"
    if f.get("lowres_mean_abs_diff_max", 0) > 12:
        return "failed", f"low-res render differs (mean abs diff {f['lowres_mean_abs_diff_max']})"
    saved = f["bytes_in"] - f["bytes_out"]
    share = saved / max(f["bytes_in"], 1)
    q = st.get("quality") or {}
    if q.get("ssim_struct_min_kobo") is None:
        return "failed", "no quality measurement"
    if q["ssim_struct_min_kobo"] < RULE["min_ssim_struct_kobo"]:
        return "not_worth", f"Kobo-size structural SSIM {q['ssim_struct_min_kobo']} < {RULE['min_ssim_struct_kobo']}"
    if q.get("ssim_struct_min_zoom2") is not None and q["ssim_struct_min_zoom2"] < RULE["min_ssim_struct_zoom2"]:
        return "not_worth", f"2x structural SSIM {q['ssim_struct_min_zoom2']} < {RULE['min_ssim_struct_zoom2']}"
    if share < RULE["min_saving_share"] or saved / 1e6 < RULE["min_saving_mb"]:
        return "not_worth", f"saves {saved / 1e6:.1f} MB ({share:.0%})"
    return "done", f"saves {saved / 1e6:.1f} MB ({share:.0%})"


# ------------------------------------------------------------------------------------------
# shrink.py
# ------------------------------------------------------------------------------------------

def local_shrink(src: Path, params: dict, outdir: Path, *, timeout: float = 6 * 3600) -> dict:
    """shrink_worker.py on src; results in outdir (out.pdf, report.json).

    shrink.local_shrink ran it as `docker run --memory 900m --cpus 1 ... jbarlow83/ocrmypdf-ubuntu`
    with the work folder as the working directory; this container is that image, so it runs as a
    child process with the same working directory, environment and single-threaded settings."""
    src = src.resolve()
    outdir.mkdir(parents=True, exist_ok=True)
    for f in ("out.pdf", "report.json", "stage1.pdf"):
        (outdir / f).unlink(missing_ok=True)
    shutil.rmtree(outdir / "tmp", ignore_errors=True)
    (outdir / "tmp").mkdir()
    (outdir / "params.json").write_text(json.dumps(dict(params, jobs=1)))
    env = dict(os.environ, SHRINK_IN=str(src), TMPDIR=str(outdir / "tmp"), OMP_THREAD_LIMIT="1",
               OMP_NUM_THREADS="1", OPENBLAS_NUM_THREADS="1")
    errf = outdir / "_stderr.log"
    t0 = time.time()
    with errf.open("w") as ferr:
        proc = subprocess.Popen([sys.executable, str(HERE / "shrink_worker.py")], cwd=outdir, env=env,
                                stdout=ferr, stderr=ferr)
        try:
            rc = proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
            rc = -9
    secs = round(time.time() - t0, 1)
    shutil.rmtree(outdir / "tmp", ignore_errors=True)
    (outdir / "stage1.pdf").unlink(missing_ok=True)
    err = errf.read_text(errors="replace")
    rep = {}
    if (outdir / "report.json").exists():
        rep = json.loads((outdir / "report.json").read_text())
    ok = rc == 0 and (outdir / "out.pdf").exists() and "text_pages_identical" in rep
    # the largest child so far (MB): shrink.py read the step container's memory.peak instead
    peak = round(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss / 1024)
    return {"rc": rc, "ok": ok, "seconds": secs, "peak_memory_mb": peak, "report": rep,
            "stderr_tail": err.strip().splitlines()[-6:]}


def excerpt(src: Path, pages: list[int], dst: Path) -> None:
    import pikepdf
    with pikepdf.open(src) as pdf:
        out = pikepdf.new()
        for i in pages:
            out.pages.append(pdf.pages[i])
        for page in out.pages:
            for key in ("/Annots", "/B", "/StructParents"):
                if key in page.obj:
                    del page.obj[key]
        out.save(dst)


# ------------------------------------------------------------------------------------------
# ocr_queue.py
# ------------------------------------------------------------------------------------------

_WORDS: set[str] | None = None


def words() -> set[str]:
    global _WORDS
    if _WORDS is None:
        _WORDS = set()
        for lang in ("es", "en", "fr", "de", "it", "pt"):
            p = HERE / "wordlists" / f"{lang}.txt.gz"
            if p.exists():
                with gzip.open(p, "rt", encoding="utf-8") as fh:
                    _WORDS.update(w for w in fh.read().split("\n") if w)
    return _WORDS


def verify(src: Path, out: Path, dpi: int = 50) -> dict:
    import numpy as np
    import pymupdf

    tok_re = re.compile(r"[^\W\d_]{2,}")
    res: dict = {"bytes_before": src.stat().st_size, "bytes_after": out.stat().st_size}
    res["ratio"] = round(res["bytes_after"] / res["bytes_before"], 4)
    a, b = pymupdf.open(src), pymupdf.open(out)
    res["pages_before"], res["pages_after"] = a.page_count, b.page_count
    t1 = [a[i].get_text() for i in range(a.page_count)]
    t2 = [b[i].get_text() for i in range(b.page_count)]
    letters = lambda t: sum(1 for c in t if c.isalpha())  # noqa: E731
    has1 = [letters(t) >= 40 for t in t1]
    has2 = [letters(t) >= 40 for t in t2]
    any2 = [letters(t) > 0 for t in t2]
    n = len(t2)
    res["text_pages_before"], res["text_pages_after"] = sum(has1), sum(has2)
    res["any_text_pages_after"] = sum(any2)
    res["pages_without_text_after"] = [i + 1 for i, h in enumerate(any2) if not h][:40]
    changed = [i for i in range(min(len(t1), n)) if t1[i] != t2[i]]
    res["pages_text_changed"] = len(changed)
    tok = known = 0
    W = words()
    for i in changed or range(n):
        for w in tok_re.findall(unicodedata.normalize("NFKC", t2[i])):
            tok += 1
            known += w.lower() in W
    res["known_word_share"] = round(known / tok, 4) if tok else None
    mid = (changed or list(range(n)))[len(changed or list(range(n))) // 2] if n else 0
    res["text_sample_page"] = mid + 1
    res["text_sample"] = re.sub(r"\s+", " ", t2[mid] if n else "")[:400]
    rng = random.Random(1234)
    pages = list(range(min(3, n)))
    pool = [i for i in changed if i not in pages]
    pages += sorted(rng.sample(pool, min(3, len(pool))))
    diffs, identical = [], True
    for i in pages:
        x = np.frombuffer(a[i].get_pixmap(dpi=dpi).samples, dtype=np.uint8)
        y = np.frombuffer(b[i].get_pixmap(dpi=dpi).samples, dtype=np.uint8)
        if x.shape != y.shape:
            identical = False
            diffs.append({"page": i + 1, "shape": [x.shape[0], y.shape[0]]})
            continue
        d = np.abs(x.astype(np.int16) - y.astype(np.int16))
        if d.max() > 0:
            identical = False
            diffs.append({"page": i + 1, "max": int(d.max()), "share": round(float((d > 0).mean()), 6)})
    res["render_pages"] = [p + 1 for p in pages]
    res["render_identical"] = identical
    res["render_diffs"] = diffs
    a.close()
    b.close()
    return res
