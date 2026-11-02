#!/usr/bin/env python3
# Vendored from book-curator (/mnt/data2/Code/book-curator, HEAD 1968187 on 2026-11-02):
#   data/acquisition/pdf-ocr/map_pdfs.py (sha256 a45466db10ed, 2026-10-26), unchanged below this header.
# The file is not tracked in book-curator's git (data/ is ignored), hence the hash. Used by the Stump
# PDF worker container (docker/Dockerfile.pdf-worker; scripts/fork/workers/README.md).
"""Phase A: map every PDF in the library (read only) and class its text layer.

Two steps, both resumable:

  scan      one subprocess per PDF (timeout, crash-safe) writes cache/map/<id>.json with
            per-page measurements (text, words known to a word list, garbage characters,
            image coverage and effective DPI, visible vs invisible text on sampled pages).
            Files whose size/mtime did not change are not read again.
  classify  reads the cache, applies the thresholds (see THRESHOLDS) and writes
            pdf_map.csv (the brief's columns) and pdf_map_detail.csv (every metric).

Nothing in the library is written: PDFs are opened read-only with PyMuPDF.

    .venv/bin/python data/acquisition/pdf-ocr/map_pdfs.py scan --jobs 5
    .venv/bin/python data/acquisition/pdf-ocr/map_pdfs.py classify
    .venv/bin/python data/acquisition/pdf-ocr/map_pdfs.py one <pdf> [--sample N]   (debug)
"""

from __future__ import annotations

import argparse
import csv
import gzip
import hashlib
import json
import math
import os
import re
import statistics
import subprocess
import sys
import time
import unicodedata
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
# Paths can be moved with environment variables: on l5 the map runs in the tools container
# with PDFOCR_LIB_ROOT=/ocr/in (a copy of the library's PDFs, same relative paths),
# PDFOCR_CATALOGUE=/ocr/catalogo.csv, PDFOCR_CACHE=/ocr/map/cache, PDFOCR_OUT=/ocr/map,
# PDFOCR_WORDLISTS=/ocr/wordlists.
LIB_ROOT = Path(os.environ.get("PDFOCR_LIB_ROOT", "/mnt/data2/Books/Biblioteca-Ebooks"))
LIB = LIB_ROOT / "biblioteca"
CATALOGUE = Path(os.environ.get("PDFOCR_CATALOGUE", str(LIB_ROOT / "catalogo.csv")))
CACHE = Path(os.environ.get("PDFOCR_CACHE", str(HERE / "cache" / "map")))
OUTDIR = Path(os.environ.get("PDFOCR_OUT", str(HERE)))
WORDLISTS = Path(os.environ.get("PDFOCR_WORDLISTS", str(HERE / "wordlists")))
PROBE_DIR = Path(os.environ.get("PDFOCR_PROBE", str(HERE / "cache" / "probe")))
CACHE_VERSION = 3
PRESAMPLE = 120            # pages probed first in a long file
PRESAMPLE_MIN_PAGES = 250  # files up to this many pages are always read in full

# ----------------------------------------------------------------------------------------
# text measurements
# ----------------------------------------------------------------------------------------

PUNCT = set(".,;:!?¡¿'\"()[]{}<>-‐‑‒–—―_/\\|@#$%&*+=^~`´¨°ºª§¶©®™€£¥¢…·•«»‹›“”‘’„‚′″")
MATH = set("±×÷√∛∞≈≠≡≤≥≪≫∫∬∮∑∏∂∇∆∈∉∋⊂⊃⊆⊇∪∩∧∨¬∀∃∄∅→←↑↓↔⇒⇐⇔↦∘∙⋅∼∝⊥∥∠∡⊕⊗⊙ℝℕℤℚℂ"
           "ℓℏℑℜ℘∗†‡⟨⟩⌈⌉⌊⌋‖∓≅≃≲≳⊢⊨∴∵")
GREEK = set("αβγδεζηθικλμνξοπρστυφχψωϑϕϵΓΔΘΛΞΠΣΥΦΨΩ")
STRIP = "".join(sorted(PUNCT | MATH)) + "0123456789"
MOJIBAKE = re.compile("Ã[\u0080-\u00bf]|Â[\u00a0-\u00bf]|â€")
TOKEN = re.compile(r"\S+")
ALPHA_TOKEN = re.compile(r"^[^\W\d_]+(?:['’\-][^\W\d_]+)*$")

# Distinctive function words per language (ambiguous ones such as de/la/en/a/in/que left out
# where they would decide between two candidate languages).
STOP = {
    "es": "el los las del y por para con una su se es lo como más pero sus al este esta "
          "también fue ha son muy sobre entre cuando sin hay ser donde porque desde todo "
          "fueron había puede cada otros".split(),
    "en": "the of and to is that for it with was on be by this are from at which or an not "
          "have his but were they their has been we can these its there would will".split(),
    "fr": "le les des et est une dans du pour qui pas au sur il sont avec ce cette mais ou "
          "aux ne plus nous été être".split(),
    "de": "der die und das ist nicht mit den von zu sich des auf ein eine dem im für auch "
          "wird sind oder aus bei nach wie".split(),
    "it": "il di che è della per sono del gli nel con non si alla dei delle più anche "
          "questo essere".split(),
    "pt": "os do da dos das não uma em com é ao mais na no também são foi pelo".split(),
    "la": "et est non ad cum quod sed ut qui quae enim autem esse sunt quam vel ab ex sic "
          "nec atque etiam".split(),
}
STOP_SETS = {k: set(v) for k, v in STOP.items()}

MATH_FONT = re.compile(r"(?i)(cmmi|cmsy|cmex|msam|msbm|eufm|rsfs|mathematicalpi|mt ?extra|"
                       r"mtsy|mtmi|euclid|stix|cambria ?math|symbol|esint|txsy|pxsy|lms|lmm)")


def load_words() -> tuple[set[str], dict[str, set[str]]]:
    per: dict[str, set[str]] = {}
    for path in sorted(WORDLISTS.glob("*.txt.gz")):
        lang = path.name.split(".")[0]
        with gzip.open(path, "rt", encoding="utf-8") as fh:
            per[lang] = {w for w in fh.read().split("\n") if w}
    esen = per.get("es", set()) | per.get("en", set())
    return esen, per


def is_cjk(ch: str) -> bool:
    o = ord(ch)
    return 0x3040 <= o <= 0x30FF or 0x4E00 <= o <= 0x9FFF or 0xFF66 <= o <= 0xFF9F


def measure_text(text: str, esen: set[str], per: dict[str, set[str]], stop: Counter,
                 rev: list[int], other: Counter) -> list[int]:
    """Per-page counts: see FIELDS."""
    t = unicodedata.normalize("NFKC", text)
    nonspace = letters = digits = garbage = fffd = pua = mathc = cjk = 0
    for ch in t:
        if ch.isspace():
            continue
        nonspace += 1
        o = ord(ch)
        if ch == "\ufffd":
            fffd += 1
            garbage += 1
        elif 0xE000 <= o <= 0xF8FF or o >= 0xF0000:
            pua += 1
            garbage += 1
        elif ch.isalpha():
            letters += 1
            if ch in GREEK:
                mathc += 1
            elif o > 0x2FFF and is_cjk(ch):
                cjk += 1
        elif ch.isdigit():
            digits += 1
        elif ch in MATH:
            mathc += 1
        elif ch in PUNCT:
            pass
        elif unicodedata.category(ch) in ("Sm",):
            mathc += 1
        else:
            garbage += 1
    alpha_tok = known = known_any = junk_tok = single_junk = long_tok = 0
    mojibake = len(MOJIBAKE.findall(t))
    for raw in TOKEN.findall(t):
        tok = raw.strip(STRIP)
        if not tok:
            continue
        if ALPHA_TOKEN.match(tok):
            low = tok.lower()
            if len(tok) == 1:
                if low not in "aeiouyx":
                    single_junk += 1
                continue
            if len(tok) > 25:
                long_tok += 1
            alpha_tok += 1
            for lang, words in STOP_SETS.items():
                if low in words:
                    stop[lang] += 1
            if low in esen or low.replace("’", "'") in esen:
                known += 1
                known_any += 1
            else:
                hit = False
                for lang, words in per.items():
                    if low in words:
                        hit = True
                        other[lang] += 1
                        break
                if hit:
                    known_any += 1
            if len(rev) < 2 or rev[1] < 3000:
                # reversed-text check on a sample of tokens: [forward known, tokens, reversed known]
                if len(rev) < 3:
                    rev[:] = [0, 0, 0]
                rev[1] += 1
                if low in esen:
                    rev[0] += 1
                if low[::-1] in esen:
                    rev[2] += 1
        else:
            # a token with letters mixed with odd characters ("t~e", "|n", "w0rd") or pure junk
            has_alpha = any(c.isalpha() for c in tok)
            odd = any(not (c.isalnum() or c in PUNCT or c in MATH or c in "'’") for c in tok)
            if has_alpha and odd:
                junk_tok += 1
            elif not has_alpha and odd:
                junk_tok += 1
    return [nonspace, letters, digits, garbage, fffd, pua, mathc, cjk, alpha_tok, known,
            known_any, junk_tok, single_junk, long_tok, mojibake]


TEXT_FIELDS = ["nonspace", "letters", "digits", "garbage", "fffd", "pua", "math", "cjk",
               "alpha_tok", "known", "known_any", "junk_tok", "single_junk", "long_tok",
               "mojibake"]
PAGE_FIELDS = TEXT_FIELDS + ["img_cov", "img_dpi", "n_img", "err"]


def image_stats(page, area: float) -> tuple[float, float, int]:
    """(share of the page covered by images, effective DPI of the best big image, count)."""
    cov = 0.0
    best_dpi = 0.0
    infos = page.get_image_info()
    rect = page.rect
    for info in infos:
        x0, y0, x1, y1 = info["bbox"]
        x0, y0 = max(x0, rect.x0), max(y0, rect.y0)
        x1, y1 = min(x1, rect.x1), min(y1, rect.y1)
        if x1 <= x0 or y1 <= y0:
            continue
        a = (x1 - x0) * (y1 - y0)
        cov += a / area
        bw, bh = info["bbox"][2] - info["bbox"][0], info["bbox"][3] - info["bbox"][1]
        if a / area >= 0.25 and bw > 0 and bh > 0 and info["width"] and info["height"]:
            dpi = math.sqrt(info["width"] * info["height"] / ((bw / 72) * (bh / 72)))
            best_dpi = max(best_dpi, dpi)
    return min(cov, 1.0), best_dpi, len(infos)


def analyse(path: Path, sample: int = 0) -> dict:
    import pymupdf

    esen, per = load_words()
    st = path.stat()
    out: dict = {"version": CACHE_VERSION, "path": str(path.relative_to(LIB_ROOT)),
                 "bytes": st.st_size, "mtime": int(st.st_mtime), "sampled": 0}
    t0 = time.time()
    try:
        doc = pymupdf.open(path)
    except Exception as exc:  # noqa: BLE001
        out.update(open_error=f"{type(exc).__name__}: {exc}"[:300], pages=0)
        return out
    out["needs_pass"] = bool(doc.needs_pass)
    out["is_encrypted"] = bool(doc.is_encrypted)
    meta = doc.metadata or {}
    out["producer"] = (meta.get("producer") or "").strip()
    out["creator"] = (meta.get("creator") or "").strip()
    out["format"] = meta.get("format") or ""
    out["encryption"] = meta.get("encryption") or ""
    n = doc.page_count
    out["pages"] = n
    if doc.needs_pass or n == 0:
        return out
    try:
        out["pdf_xref_len"] = doc.xref_length()
    except Exception:  # noqa: BLE001
        pass
    pages = list(range(n))
    if sample and n > sample:
        step = n / sample
        pages = sorted({int(i * step) for i in range(sample)})
        out["sampled"] = len(pages)
    elif not sample and n > PRESAMPLE_MIN_PAGES:
        # Stage 1: an even sample. If no sampled page is an image page without text, the
        # file is born digital or fully OCRed and the sample is enough: a block of image
        # pages large enough to matter (>= 3% of the pages) would contain sampled pages.
        step = n / PRESAMPLE
        probe = sorted({int(i * step) for i in range(PRESAMPLE)})
        need = 0
        for pno in probe:
            try:
                page = doc.load_page(pno)
                rect = page.rect
                area = max(rect.width * rect.height, 1.0)
                letters = sum(1 for c in page.get_text("text") if c.isalpha())
                cov, _dpi, _n = image_stats(page, area)
                if letters < 40 and cov >= 0.5:
                    need += 1
            except Exception:  # noqa: BLE001
                need += 1
        if need == 0:
            pages = probe
            out["sampled"] = len(pages)
    trace_pages = set(pages[:: max(1, len(pages) // 12)][:12])
    stop: Counter = Counter()
    other: Counter = Counter()
    rev: list[int] = []
    rows = []
    vis = inv = 0
    fonts: Counter = Counter()
    snippets = []
    rot = Counter()
    sizes = Counter()
    for pno in pages:
        try:
            page = doc.load_page(pno)
            rect = page.rect
            area = max(rect.width * rect.height, 1.0)
            rot[page.rotation] += 1
            sizes[(round(rect.width), round(rect.height))] += 1
            text = page.get_text("text")
            row = measure_text(text, esen, per, stop, rev, other)
            cov, dpi, nimg = image_stats(page, area)
            row += [round(cov, 3), round(dpi, 1), nimg, 0]
            if pno in trace_pages:
                for span in page.get_texttrace():
                    k = len(span["chars"])
                    if span["type"] == 3 or span.get("opacity", 1) == 0:
                        inv += k
                    else:
                        vis += k
                for f in page.get_fonts():
                    fonts[f"{f[2]}:{f[3]}"] += 1
            if row[1] >= 200 and len(snippets) < 4 and pno >= min(6, n // 4):
                snippets.append([pno, re.sub(r"\s+", " ", text)[:400]])
        except Exception as exc:  # noqa: BLE001
            row = [0] * len(TEXT_FIELDS) + [0.0, 0.0, 0, 1]
            out.setdefault("page_errors", []).append([pno, f"{type(exc).__name__}: {exc}"[:160]])
        rows.append([pno] + row)
    out["page_fields"] = ["pno"] + PAGE_FIELDS
    out["rows"] = rows
    out["stop"] = dict(stop)
    out["other_lang_hits"] = dict(other)
    out["reversed"] = rev
    out["trace_visible"] = vis
    out["trace_invisible"] = inv
    out["fonts"] = dict(fonts.most_common(40))
    out["snippets"] = snippets
    out["rotations"] = {str(k): v for k, v in rot.items()}
    out["page_sizes"] = [[list(k), v] for k, v in sizes.most_common(5)]
    try:
        toc = doc.get_toc(simple=True)
        out["toc_len"] = len(toc)
    except Exception:  # noqa: BLE001
        out["toc_len"] = -1
    out["seconds"] = round(time.time() - t0, 2)
    return out


def cache_name(rel: str) -> Path:
    return CACHE / (hashlib.sha1(rel.encode()).hexdigest()[:16] + ".json")


def list_pdfs() -> list[Path]:
    return sorted(p for p in LIB.rglob("*") if p.is_file() and p.suffix.lower() == ".pdf")


def cmd_one(args) -> int:
    path = Path(args.pdf).resolve()
    res = analyse(path, sample=args.sample)
    data = json.dumps(res, ensure_ascii=False)
    if args.out:
        tmp = Path(args.out + ".tmp")
        tmp.write_text(data, encoding="utf-8")
        os.replace(tmp, args.out)
    else:
        print(data[:3000])
    return 0


def cmd_scan(args) -> int:
    CACHE.mkdir(parents=True, exist_ok=True)
    pdfs = list_pdfs()
    if args.limit:
        pdfs = pdfs[: args.limit]
    todo = []
    for p in pdfs:
        rel = str(p.relative_to(LIB_ROOT))
        c = cache_name(rel)
        if c.exists() and not args.force:
            try:
                old = json.loads(c.read_text(encoding="utf-8"))
                st = p.stat()
                if (old.get("version") == CACHE_VERSION and old.get("bytes") == st.st_size
                        and old.get("mtime") == int(st.st_mtime)):
                    continue
            except (OSError, ValueError):
                pass
        todo.append(p)
    # big files first so the long ones overlap with many small ones
    todo.sort(key=lambda p: -p.stat().st_size)
    print(f"{len(pdfs)} PDFs, {len(todo)} to scan", flush=True)
    log = (CACHE.parent / "scan.log").open("a", encoding="utf-8")

    def run(p: Path) -> None:
        rel = str(p.relative_to(LIB_ROOT))
        out = cache_name(rel)
        base = ["nice", "-n", "19", "ionice", "-c", "3", sys.executable, __file__, "one",
                str(p), "--out", str(out)]
        for sample, timeout in ((0, args.timeout), (150, args.timeout)):
            cmd = base + (["--sample", str(sample)] if sample else [])
            t0 = time.time()
            try:
                r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
                if r.returncode == 0 and out.exists():
                    log.write(f"ok\t{rel}\t{time.time()-t0:.1f}s\tsample={sample}\n")
                    log.flush()
                    return
                err = (r.stderr or "")[-300:].replace("\n", " ")
                log.write(f"fail\t{rel}\trc={r.returncode}\t{err}\n")
            except subprocess.TimeoutExpired:
                log.write(f"timeout\t{rel}\t{timeout}s\tsample={sample}\n")
            log.flush()
        out.write_text(json.dumps({"version": CACHE_VERSION, "path": rel,
                                   "bytes": p.stat().st_size, "mtime": int(p.stat().st_mtime),
                                   "open_error": "analysis crashed or timed out", "pages": 0}),
                       encoding="utf-8")

    done = 0
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for _ in pool.map(run, todo):
            done += 1
            if done % 25 == 0:
                print(f"  {done}/{len(todo)}", flush=True)
    print("scan finished", flush=True)
    return 0


# ----------------------------------------------------------------------------------------
# classification
# ----------------------------------------------------------------------------------------

THRESHOLDS = {
    # a page "has text" when it has at least this many letters (or CJK characters / 2)
    "page_text_letters": 40,
    # an image covering at least this share of the page makes it a scanned/image page
    "page_image_cov": 0.5,
    # no_text: share of pages with text below this
    "no_text_cov": 0.05,
    # partial: image pages without text at least this many and this share of all pages
    "partial_min_pages": 5,
    "partial_min_share": 0.02,
    # poor_ocr: doc-level share of known words (quality_score) below this ...
    "poor_quality": 0.80,
    # ... or this share of text pages with page quality below bad_page_quality
    "bad_page_quality": 0.60,
    "poor_bad_page_share": 0.30,
    # broken: page errors on more than this share of pages
    "broken_err_share": 0.10,
    # OCR probe: a sampled image page "shows text" when Tesseract finds this many dictionary words
    "probe_text_known": 60,
    # OCR-error signature (ocr_signature.py): share of tokens with a capital or digit inside a
    # lowercase word ("razOn", "inspiraci6n"); good OCR layers stay below 1.5%
    "poor_signature": 0.04,
}

MATH_TAGS = re.compile(r"(?i)matem|física|fisica|physics|química|quimica|chemistry|estad|"
                       r"cálculo|calculo|calculus|álgebra|algebra|ingenier|engineering|"
                       r"electr|mechanic|mecánica|termodin|probab")
MATH_TITLE = re.compile(r"(?i)calcul|cálculo|álgebra|algebra|analysis|análisis|mathemat|"
                        r"matemát|matemat|physic|física|fisica|equation|ecuacion|geometr|"
                        r"trigonom|mechanic|mecánica|probab|statist|estadíst|topolog|"
                        r"differential|diferencial|integral|vector|tensor|quantum|cuántica|"
                        r"thermodynam|termodin|electromagnet|chemistry|química|quimica|"
                        r"problems|problemas")


def load_catalogue() -> dict[str, dict]:
    with CATALOGUE.open(encoding="utf-8") as fh:
        return {row["path"]: row for row in csv.DictReader(fh)}


def detect_language(stop: dict, cjk_share: float) -> tuple[str, dict]:
    if cjk_share > 0.2:
        return "ja", {}
    total = sum(stop.values())
    if total < 30:
        return "", {}
    ranked = sorted(stop.items(), key=lambda kv: -kv[1])
    top, n1 = ranked[0]
    langs = [top]
    if len(ranked) > 1 and ranked[1][1] >= 0.35 * n1 and ranked[1][1] >= 30:
        langs.append(ranked[1][0])
    return "+".join(langs), {k: round(v / total, 3) for k, v in ranked[:3]}


def summarise(d: dict, cat: dict) -> dict:
    T = THRESHOLDS
    rel = d["path"]
    crow = cat.get(rel, {})
    res = {"path": rel, "pages": d.get("pages", 0), "bytes": d.get("bytes", 0),
           "producer": d.get("producer", ""), "creator": d.get("creator", ""),
           "cat_language": crow.get("language", ""), "title": crow.get("title", ""),
           "estante": crow.get("estante", ""), "tags": crow.get("tags", "")}
    notes = []
    if d.get("open_error"):
        res.update(cls="broken", born_digital="", text_coverage="", quality_score="",
                   dpi_median="", language=crow.get("language", ""),
                   notes=f"cannot open: {d['open_error']}")
        return res
    if d.get("needs_pass"):
        res.update(cls="broken", born_digital="", text_coverage="", quality_score="",
                   dpi_median="", language=crow.get("language", ""),
                   notes="password protected (user password)")
        return res
    fields = d["page_fields"]
    ix = {f: i for i, f in enumerate(fields)}
    rows = d["rows"]
    n = len(rows)
    if n == 0:
        res.update(cls="broken", notes="no pages", born_digital="", text_coverage="",
                   quality_score="", dpi_median="", language="")
        return res
    err = sum(r[ix["err"]] for r in rows)
    has_text = [r[ix["letters"]] - r[ix["cjk"]] + 2 * r[ix["cjk"]] >= T["page_text_letters"]
                for r in rows]
    little_text = [0 < r[ix["nonspace"]] and not h for r, h in zip(rows, has_text)]
    img_page = [r[ix["img_cov"]] >= T["page_image_cov"] for r in rows]
    need = [ip and not h and not r[ix["err"]] for r, ip, h in zip(rows, img_page, has_text)]
    blank = [not ip and not h for ip, h in zip(img_page, has_text)]
    n_text = sum(has_text)
    n_img = sum(img_page)
    n_need = sum(need)
    cov = n_text / n
    # quality over text pages
    alpha = sum(r[ix["alpha_tok"]] for r, h in zip(rows, has_text) if h)
    known = sum(r[ix["known"]] for r, h in zip(rows, has_text) if h)
    known_any = sum(r[ix["known_any"]] for r, h in zip(rows, has_text) if h)
    junk = sum(r[ix["junk_tok"]] for r, h in zip(rows, has_text) if h)
    nonspace = sum(r[ix["nonspace"]] for r in rows) or 1
    garbage = sum(r[ix["garbage"]] for r in rows)
    fffd = sum(r[ix["fffd"]] for r in rows)
    pua = sum(r[ix["pua"]] for r in rows)
    mathc = sum(r[ix["math"]] for r in rows)
    cjk = sum(r[ix["cjk"]] for r in rows)
    letters = sum(r[ix["letters"]] for r in rows) or 1
    mojibake = sum(r[ix["mojibake"]] for r in rows)
    single = sum(r[ix["single_junk"]] for r, h in zip(rows, has_text) if h)
    lang, shares = detect_language(d.get("stop", {}), cjk / letters)
    use_any = bool(lang) and lang.split("+")[0] not in ("es", "en")
    k = known_any if use_any else known
    quality = k / (alpha + junk) if (alpha + junk) else None
    # per-page quality
    bad_pages = 0
    q_pages = 0
    for r, h in zip(rows, has_text):
        if not h:
            continue
        a = r[ix["alpha_tok"]] + r[ix["junk_tok"]]
        if a < 20:
            continue
        q_pages += 1
        kk = r[ix["known_any"]] if use_any else r[ix["known"]]
        if kk / a < T["bad_page_quality"]:
            bad_pages += 1
    bad_share = bad_pages / q_pages if q_pages else 0.0
    dpis = [r[ix["img_dpi"]] for r, ip in zip(rows, img_page) if ip and r[ix["img_dpi"]] > 0]
    dpi_median = round(statistics.median(dpis)) if dpis else ""
    vis, inv = d.get("trace_visible", 0), d.get("trace_invisible", 0)
    inv_share = inv / (vis + inv) if (vis + inv) else 0.0
    img_share = n_img / n
    if inv_share >= 0.5 and img_share >= 0.5:
        born = "no"            # scan with an (invisible) OCR layer
    elif img_share >= 0.8 and cov < 0.5:
        born = "no"            # scan without text
    elif img_share >= 0.8 and inv_share < 0.5:
        born = "mixed"         # full-page images with visible text (ClearScan, photo books)
    elif img_share <= 0.3:
        born = "yes"
    else:
        born = "mixed"
    # reversed text
    rev = d.get("reversed") or [0, 0, 0]
    reversed_text = len(rev) == 3 and rev[1] > 200 and rev[2] > rev[0] + 0.2 * rev[1]
    # maths hint
    math_share = mathc / nonspace
    math_fonts = sorted({f for f in d.get("fonts", {}) if MATH_FONT.search(f)})
    tags = crow.get("tags", "") or ""
    title = crow.get("title", "") or ""
    if math_share >= 0.01 or (math_fonts and math_share >= 0.003):
        math_hint = "high"
    elif math_share >= 0.004 or math_fonts:
        math_hint = "some"
    elif MATH_TAGS.search(tags) or MATH_TITLE.search(title):
        math_hint = "likely(tags)" if n_text < 0.5 * n else "low"
    else:
        math_hint = "low"
    # --- class (thresholds calibrated by eye; see research/pdf-ocr.md) ---
    cjk_share = cjk / letters
    unsupported = shares.get("la", 0) if shares else 0
    is_cjk = cjk_share > 0.05 or (crow.get("language") or "") in ("ja", "zh")
    ocr_layer = inv_share >= 0.5 or (img_share >= 0.8 and n_text > 0)
    # one space between every letter (e.g. an ABBYY layer of letter-spaced type): single-letter
    # "words" outnumber real ones several times over. Maths (x, y, n variables) stays below 3.
    letter_spaced = alpha > 0 and single / alpha > 3.0 and not is_cjk
    need_little = sum(1 for r, nd in zip(rows, need) if nd and r[ix["nonspace"]] > 0)
    probe = d.get("_probe") or {}
    sig = (d.get("_sig") or {}).get("share")
    probed = [pp for pp in probe.get("pages", [])]
    probe_hits = [pp for pp in probed if pp[2] >= T["probe_text_known"]]
    if probed:
        est_need_text = round(n_need * len(probe_hits) / len(probed))
    else:
        est_need_text = None
    queue = ""
    if err / n > T["broken_err_share"]:
        cls = "broken"
        notes.append(f"page errors on {err}/{n} pages")
    elif cov < T["no_text_cov"] and n_need >= max(1, 0.5 * n):
        cls = "no_text"
    elif cov < T["no_text_cov"]:
        cls = "no_text" if n_img else "broken"
        notes.append("pages have neither text nor full-page images (vector outlines?)")
    elif is_cjk and n_need >= T["partial_min_pages"] and n_need / n >= T["partial_min_share"]:
        cls = "partial"
    elif (est_need_text if est_need_text is not None else n_need) >= T["partial_min_pages"] and \
            (est_need_text if est_need_text is not None else n_need) / n >= T["partial_min_share"]:
        cls = "partial"
    elif letter_spaced:
        cls = "poor_ocr"
        notes.append("letter-spaced text layer (a space between every letter)")
    elif (quality is not None and ocr_layer and not is_cjk and unsupported < 0.15
          and quality < T["poor_quality"]):
        cls = "poor_ocr"
    elif ocr_layer and (reversed_text or (fffd + pua) / nonspace > 0.05 or mojibake > 50):
        cls = "poor_ocr"
    elif (ocr_layer and sig is not None and sig >= T["poor_signature"] and quality is not None
          and quality < 0.95 and not is_cjk):
        cls = "poor_ocr"
        notes.append(f"OCR-error signature {sig:.1%} (capital/digit inside words: "
                     f"{', '.join((d.get('_sig') or {}).get('examples', [])[:4])})")
    else:
        cls = "ok"
    # OCRmyPDF mode and whether the file goes into the default queue
    if cls in ("no_text", "partial", "poor_ocr"):
        poor_text = (quality is not None and ocr_layer and not is_cjk and unsupported < 0.15
                     and quality < T["poor_quality"])
        if letter_spaced and inv_share < 0.5:
            # text drawn under the page image (render mode 0, hidden by the image): --redo-ocr
            # keeps such "visible" text and --force-ocr re-rasterizes the pages (pilot: 4.8x the
            # size), so the text objects are removed from a copy first (strip_hidden_text.py)
            mode = "strip"
            notes.append("text hidden under the page image: strip it from a copy, then OCR")
        elif cls == "poor_ocr" or poor_text or (sig is not None and sig >= T["poor_signature"]):
            mode = "redo"
        elif need_little > max(2, 0.2 * n_need):
            mode = "redo"      # watermark/stamp text on image pages: --skip-text would skip them
        else:
            mode = "skip"
        queue = "yes"
        if is_cjk:
            queue = "no:japanese"
        elif cls == "no_text" and probed and max(pp[2] for pp in probed) < 15:
            queue = "no:probe-found-no-text"
        elif cls == "poor_ocr" and not ocr_layer:
            queue = "no:born-digital"
    else:
        mode = ""
    if probed:
        notes.append(f"probe: {len(probe_hits)}/{len(probed)} sampled image pages show text")
    if is_cjk:
        notes.append("CJK text: word-list quality not assessed")
    elif unsupported >= 0.15:
        notes.append(f"Latin {unsupported:.0%} of function words: word-list quality understated")
    if quality is not None and not ocr_layer and quality < T["poor_quality"] and cls == "ok":
        notes.append("born-digital text; low word-list share from maths/notation")
    # notes
    notes.append(f"text pages {n_text}/{n}")
    if n_need:
        notes.append(f"image pages without text {n_need}")
    if sum(little_text):
        notes.append(f"pages with only a little text {sum(little_text)}")
    if inv + vis:
        notes.append(f"invisible text {inv_share:.0%}")
    if (fffd + pua) / nonspace > 0.005:
        notes.append(f"unmapped glyphs {(fffd + pua) / nonspace:.1%}")
    if garbage / nonspace > 0.01:
        notes.append(f"garbage chars {garbage / nonspace:.1%}")
    if mojibake:
        notes.append(f"mojibake {mojibake}")
    if reversed_text:
        notes.append("reversed text")
    if bad_pages:
        notes.append(f"bad-quality text pages {bad_pages}/{q_pages}")
    notes.append(f"math:{math_hint}")
    if math_fonts:
        notes.append("math fonts " + ",".join(f.split(":", 1)[-1].split("+")[-1] for f in math_fonts[:3]))
    if d.get("is_encrypted"):
        notes.append("encrypted (owner password)")
    if d.get("sampled"):
        notes.append(f"sampled {d['sampled']} pages")
    if "ClearScan" in res["producer"]:
        notes.append("ClearScan")
    if lang and crow.get("language") and lang.split("+")[0] != crow.get("language"):
        notes.append(f"catalogue says {crow.get('language')}")
    language = lang or (crow.get("language") or "")
    if not lang and probe.get("stop"):
        # no text layer: the language Tesseract saw on the sampled pages beats the catalogue
        plang, _ = detect_language(probe["stop"], 0.0) if sum(probe["stop"].values()) >= 30 else ("", {})
        if not plang and sum(probe["stop"].values()) >= 12:
            plang = max(probe["stop"], key=probe["stop"].get)
        if plang:
            if crow.get("language") and plang.split("+")[0] != crow.get("language"):
                notes.append(f"OCR probe reads {plang}, catalogue says {crow.get('language')}")
            language = plang
    if not language:
        language = "en"
        notes.append("language unknown: defaulting to English")
    res.update(cls=cls, born_digital=born, text_coverage=round(cov, 3),
               quality_score="" if quality is None else round(quality, 3),
               dpi_median=dpi_median, language=language, notes="; ".join(notes),
               # detail
               n_text=n_text, n_img_pages=n_img, n_need_ocr=n_need, n_blank=sum(blank),
               inv_share=round(inv_share, 3), garbage_share=round(garbage / nonspace, 4),
               unmapped_share=round((fffd + pua) / nonspace, 4), math_share=round(math_share, 4),
               math_hint=math_hint, bad_pages=bad_pages, q_pages=q_pages,
               single_junk=single, alpha_tokens=alpha, lang_shares=json.dumps(shares),
               detected_language=lang, sampled=d.get("sampled", 0), mode=mode, queue=queue,
               est_need_text="" if est_need_text is None else est_need_text,
               need_little=need_little, cjk_share=round(cjk_share, 3),
               ocr_signature="" if sig is None else sig,
               encrypted=int(bool(d.get("is_encrypted"))), seconds=d.get("seconds", ""),
               dpi_min=round(min(dpis)) if dpis else "", dpi_max=round(max(dpis)) if dpis else "",
               toc_len=d.get("toc_len", ""))
    return res


MAP_COLUMNS = ["path", "pages", "bytes", "producer", "born_digital", "text_coverage",
               "quality_score", "dpi_median", "language", "class", "notes"]


def cmd_classify(args) -> int:
    cat = load_catalogue()
    sigp = CACHE.parent / "signature.json"
    sigs = json.loads(sigp.read_text(encoding="utf-8")) if sigp.exists() else {}
    out = []
    for p in list_pdfs():
        rel = str(p.relative_to(LIB_ROOT))
        c = cache_name(rel)
        if not c.exists():
            print("missing cache:", rel, file=sys.stderr)
            continue
        d = json.loads(c.read_text(encoding="utf-8"))
        if rel in sigs and sigs[rel].get("bytes") == d.get("bytes"):
            d["_sig"] = sigs[rel]
        pr = PROBE_DIR / c.name
        if pr.exists():
            pdata = json.loads(pr.read_text(encoding="utf-8"))
            if pdata.get("bytes") == d.get("bytes"):
                d["_probe"] = pdata
        out.append(summarise(d, cat))
    with (OUTDIR / "pdf_map.csv").open("w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh)
        w.writerow(MAP_COLUMNS)
        for r in out:
            w.writerow([r["path"], r["pages"], r["bytes"], r["producer"], r["born_digital"],
                        r["text_coverage"], r["quality_score"], r["dpi_median"],
                        r["language"], r["cls"], r["notes"]])
    keys = sorted({k for r in out for k in r})
    first = ["path", "cls", "pages", "bytes", "language", "cat_language", "detected_language",
             "text_coverage", "quality_score", "born_digital", "dpi_median"]
    cols = first + [k for k in keys if k not in first]
    with (OUTDIR / "pdf_map_detail.csv").open("w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=cols)
        w.writeheader()
        for r in out:
            w.writerow(r)
    cnt = Counter(r["cls"] for r in out)
    print(dict(cnt))
    for cls in ("no_text", "partial", "poor_ocr", "broken"):
        sub = [r for r in out if r["cls"] == cls]
        print(f"{cls}: {len(sub)} files, {sum(r['pages'] for r in sub)} pages, "
              f"{sum(r['bytes'] for r in sub)/1e9:.2f} GB")
    return 0


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    a = sub.add_parser("one")
    a.add_argument("pdf")
    a.add_argument("--out")
    a.add_argument("--sample", type=int, default=0)
    a.set_defaults(func=cmd_one)
    a = sub.add_parser("scan")
    a.add_argument("--jobs", type=int, default=5)
    a.add_argument("--timeout", type=int, default=900)
    a.add_argument("--limit", type=int, default=0)
    a.add_argument("--force", action="store_true")
    a.set_defaults(func=cmd_scan)
    a = sub.add_parser("classify")
    a.set_defaults(func=cmd_classify)
    args = ap.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
