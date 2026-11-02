#!/usr/bin/env python3
# Vendored from book-curator (/mnt/data2/Code/book-curator, HEAD 1968187 on 2026-11-02):
#   data/acquisition/pdf-size/shrink_worker.py (sha256 ce6cdd811bb8, 2026-10-30), unchanged below this header.
# The file is not tracked in book-curator's git (data/ is ignored), hence the hash. Used by the Stump
# PDF worker container (docker/Dockerfile.pdf-worker; scripts/fork/workers/README.md).
"""Shrink one PDF (or a page-range chunk of one) without touching its text layer.

Runs inside jbarlow83/ocrmypdf-ubuntu:v17.13.0 (pikepdf, Pillow, numpy, pypdfium2, jbig2enc and
OCRmyPDF's optimizer are all in that image), started by shrink.local_shrink on g3 as one job of the
shared g3slot pool (--memory 900m, --cpus 1, single-threaded). Input: the PDF through a read-only bind
mount (SHRINK_IN), params.json in the work folder (cwd). Output in the work folder: out.pdf,
report.json and, for the study, crops/*.png. (First drafted to run on l5 over ssh; l5 is not used.)

Stage 1 (pikepdf + Pillow, image by image), 8-bit images only:
  * downsample to params.target_dpi when the effective resolution (pixels over the size the image
    is drawn at on a page of plausible size) exceeds it by more than params.slack; images that
    cover most of a page never go below params.min_long_px on their long side;
  * colour images whose pixels are all neutral grey become grey (params.gray);
  * re-encode as baseline JPEG (params.jpeg_q) when downsampled or turned grey, and - if asked -
    page images (cover >= params.reencode_min_cover) that are JPEG (params.requality), lossless
    (params.flate_to_jpeg) or JPEG 2000 (params.jpx); a re-encode is kept only when it saves at
    least params.min_gain of that image's stored bytes; JPEG 2000 is never written;
  * params.bilevel_auto: 8-bit grey page images that are really black and white (mean change after
    an Otsu threshold <= bilevel_max_mae levels) -> 1-bit, lossless JBIG2; params.bilevel (study
    only): any grey text page -> 1-bit.
  Images with soft masks, masks, Decode arrays, CMYK/Lab/Indexed colour, >8 bits, or that are
  themselves masks are never touched.
Stage 2: OCRmyPDF's optimizer on its own (no OCR): level 1 = lossless (1-bit images -> JBIG2
  generic coding, Flate over JPEGs, object streams); "2nj" = level 2 without its blanket JPEG
  re-encoding (adds pngquant for lossless images); level 0 = skip.
Checks (run in a fresh process after an exec, so the memory of stage 1 is given back): page count;
the text of every page (pdfium) identical; renders of the requested pages at Kobo Clara HD size
(fit into 1072x1448, grey) and at twice that scale: PSNR and SSIM (whole page, and over windows that
hold ink). Production measures quality with MuPDF instead (shrink_queue.py _final).
"""

from __future__ import annotations

import io
import json
import math
import os
import resource
import subprocess
import sys
import time
import zlib
import multiprocessing as mp
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np
import pikepdf
from PIL import Image
from pikepdf import Name

Image.MAX_IMAGE_PIXELS = None
W = Path.cwd()
IN = Path(os.environ.get("SHRINK_IN") or (W / "in.pdf"))
P: dict = {}
PDF = None

DEFAULTS = {
    "target_dpi": 300, "slack": 0.2, "min_long_px": 2000, "jpeg_q": 80,
    "requality": False, "flate_to_jpeg": False, "gray": False, "gray_tol": 10, "gray_tint": False,
    "jpx": False, "reencode_min_cover": 0.0,
    "bilevel": False, "bilevel_auto": False, "pack_levels": False, "bilevel_max_mae": 3.0, "bilevel_min_ext": 0.985,
    "min_gain": 0.15, "optimize": 1, "jobs": 1,
    "metric_pages": [], "metric_auto": 6, "crops": False, "crop_pages": [], "kobo": [1072, 1448],
    "min_image_px": 300 * 300,
}

SIMPLE_CS = {"/DeviceGray": 1, "/CalGray": 1, "/DeviceRGB": 3, "/CalRGB": 3}


# ------------------------------------------------------------------------------------------
# placements: where each image XObject is drawn (CTM walk over q/Q/cm/Do)
# ------------------------------------------------------------------------------------------

def mul(m, n):
    a, b, c, d, e, f = m
    A, B, C, D, E, F = n
    return (a * A + b * C, a * B + b * D, c * A + d * C, c * B + d * D,
            e * A + f * C + E, e * B + f * D + F)


def walk(owner, resources, ctm, depth, out, pno, page_wh):
    try:
        ops = pikepdf.parse_content_stream(owner, "q Q cm Do")
    except Exception:  # noqa: BLE001
        return
    stack = []
    xo = resources.get("/XObject") if resources is not None else None
    for operands, op in ops:
        op = str(op)
        if op == "q":
            stack.append(ctm)
        elif op == "Q":
            ctm = stack.pop() if stack else ctm
        elif op == "cm" and len(operands) == 6:
            try:
                ctm = mul(tuple(float(x) for x in operands), ctm)
            except (TypeError, ValueError):
                pass
        elif op == "Do" and xo is not None and operands:
            obj = xo.get(operands[0])
            if obj is None:
                continue
            st = obj.get("/Subtype")
            if st == "/Image":
                w_pt, h_pt = math.hypot(ctm[0], ctm[1]), math.hypot(ctm[2], ctm[3])
                out.setdefault(obj.objgen, []).append((pno, w_pt, h_pt, page_wh))
            elif st == "/Form" and depth < 6:
                try:
                    m = tuple(float(x) for x in obj.get("/Matrix", [1, 0, 0, 1, 0, 0]))
                except (TypeError, ValueError):
                    m = (1, 0, 0, 1, 0, 0)
                walk(obj, obj.get("/Resources", resources), mul(m, ctm), depth + 1, out, pno, page_wh)


def placements(pdf) -> dict:
    out: dict = {}
    for pno, page in enumerate(pdf.pages):
        mb = page.mediabox
        try:
            pw, ph = abs(float(mb[2]) - float(mb[0])), abs(float(mb[3]) - float(mb[1]))
        except Exception:  # noqa: BLE001
            pw = ph = 0.0
        walk(page, page.obj.get("/Resources"), (1, 0, 0, 1, 0, 0), 0, out, pno, (pw, ph))
    return out


# ------------------------------------------------------------------------------------------
# per-image work (in worker processes)
# ------------------------------------------------------------------------------------------

def init_worker(params):
    global PDF, P
    P = params
    PDF = pikepdf.open(IN)


def filters(obj) -> list[str]:
    f = obj.get("/Filter")
    if f is None:
        return []
    if isinstance(f, pikepdf.Array):
        return [str(x) for x in f]
    return [str(f)]


def colours(obj) -> int:
    cs = obj.get("/ColorSpace")
    if isinstance(cs, pikepdf.Name):
        return SIMPLE_CS.get(str(cs), 0)
    if isinstance(cs, pikepdf.Array) and len(cs) >= 2 and str(cs[0]) == "/ICCBased":
        try:
            n = int(cs[1].get("/N", 0))
        except Exception:  # noqa: BLE001
            return 0
        return n if n in (1, 3) else 0
    return 0


def eligible(obj, smasks) -> str | None:
    """None if this 8-bit image may be re-coded, else the reason it is left alone."""
    if obj.objgen in smasks:
        return "is a mask"
    for k in ("/SMask", "/Mask", "/Decode", "/ImageMask", "/SMaskInData"):
        if k in obj:
            if k == "/ImageMask" and not bool(obj.get(k)):
                continue
            if k == "/SMaskInData" and not int(obj.get(k) or 0):
                continue
            return f"has {k}"
    f = filters(obj)
    if f == ["/JPXDecode"]:
        if not P.get("jpx"):
            return "filter JPXDecode"
        cs = obj.get("/ColorSpace")
        if cs is not None and not colours(obj):
            return "colour space"
        if int(obj.get("/Width", 0)) * int(obj.get("/Height", 0)) < P["min_image_px"]:
            return "small"
        return None
    try:
        if int(obj.get("/BitsPerComponent", 0)) != 8:
            return "not 8-bit"
    except Exception:  # noqa: BLE001
        return "odd bpc"
    if not f or not set(f) <= {"/FlateDecode", "/DCTDecode"}:
        return "filter " + "+".join(f or ["none"])
    if "/DCTDecode" in f and f[-1] != "/DCTDecode":
        return "odd filter order"
    if not colours(obj):
        return "colour space"
    if int(obj.get("/Width", 0)) * int(obj.get("/Height", 0)) < P["min_image_px"]:
        return "small"
    return None


def decode(obj) -> Image.Image:
    f = filters(obj)
    n = colours(obj)
    w, h = int(obj.Width), int(obj.Height)
    if f == ["/JPXDecode"]:
        im = Image.open(io.BytesIO(obj.read_raw_bytes()))
        im.load()
        if im.mode not in ("L", "RGB") or im.size != (w, h):
            raise ValueError(f"jpx mode {im.mode} {im.size}")
        if n and (n == 1) != (im.mode == "L"):
            raise ValueError("jpx colour space mismatch")
        return im
    if "/DCTDecode" in f:
        data = obj.read_raw_bytes()
        if f == ["/FlateDecode", "/DCTDecode"]:
            dp = obj.get("/DecodeParms")
            if isinstance(dp, pikepdf.Array) and len(dp) and isinstance(dp[0], pikepdf.Dictionary) \
                    and int(dp[0].get("/Predictor", 1)) > 1:
                raise ValueError("Flate predictor over JPEG")
            data = zlib.decompress(data)
        im = Image.open(io.BytesIO(data))
        im.load()
        if im.mode not in ("L", "RGB"):
            raise ValueError(f"jpeg mode {im.mode}")
        if (n == 1) != (im.mode == "L"):
            im = im.convert("L" if n == 1 else "RGB")
        return im
    raw = obj.read_bytes()
    mode = "L" if n == 1 else "RGB"
    if len(raw) < w * h * n:
        raise ValueError("short image data")
    return Image.frombytes(mode, (w, h), raw[: w * h * n])


def grey_kind(im: Image.Image, tol: int) -> str:
    """'grey' when no pixel carries colour, 'tint' when the only colour is a uniform paper tint,
    '' otherwise (any coloured word, line, photo or map keeps the image in colour)."""
    small = im.copy()
    small.thumbnail((1000, 1000))
    ycc = np.asarray(small.convert("YCbCr"), dtype=np.int16)
    cb, cr = ycc[..., 1], ycc[..., 2]
    dev = np.maximum(np.abs(cb - 128), np.abs(cr - 128))
    if (dev > 2.5 * tol).mean() < 0.0002 and float(np.percentile(dev, 99)) <= tol:
        return "grey"
    mcb, mcr = np.median(cb), np.median(cr)
    rel = np.maximum(np.abs(cb - mcb), np.abs(cr - mcr))
    if max(abs(mcb - 128), abs(mcr - 128)) <= 3 * tol and (rel > 2.5 * tol).mean() < 0.0002 \
            and float(np.percentile(rel, 99)) <= tol:
        return "tint"
    return ""


def text_like(im: Image.Image) -> bool:
    small = im.copy()
    small.thumbnail((1000, 1000))
    a = np.asarray(small.convert("L"))
    mid = ((a > 70) & (a < 185)).mean()
    dark = (a <= 70).mean()
    return mid < 0.06 and 0.005 < dark < 0.35


def pack_bits(idx: np.ndarray, bits: int) -> bytes:
    """Rows of small integers (< 2**bits) packed MSB first, each row padded to a whole byte (PDF layout)."""
    h, w = idx.shape
    per = 8 // bits
    pad = (-w) % per
    if pad:
        idx = np.pad(idx, ((0, 0), (0, pad)))
    idx = idx.reshape(h, -1, per).astype(np.uint8)
    out = np.zeros(idx.shape[:2], np.uint8)
    for k in range(per):
        out |= (idx[:, :, k] << (8 - bits * (k + 1))).astype(np.uint8)
    return out.tobytes()


def otsu(a: np.ndarray) -> int:
    hist = np.bincount(a.ravel(), minlength=256).astype(np.float64)
    p = hist / hist.sum()
    omega = np.cumsum(p)
    mu = np.cumsum(p * np.arange(256))
    mt = mu[-1]
    with np.errstate(divide="ignore", invalid="ignore"):
        sb = (mt * omega - mu) ** 2 / (omega * (1 - omega))
    sb = np.nan_to_num(sb)
    return int(np.argmax(sb))


def jbig2_bytes(bw: Image.Image, tag: str) -> bytes:
    png = W / f"bl_{tag}.png"
    bw.save(png)
    r = subprocess.run(["jbig2", "--pdf", str(png)], capture_output=True, check=True)
    png.unlink()
    return r.stdout


def work(item):
    og, pl = item
    t0 = time.time()
    obj = PDF.get_object(og)
    old = len(obj.read_raw_bytes())
    w, h = int(obj.Width), int(obj.Height)
    rec = {"objgen": list(og), "w": w, "h": h, "old": old, "filter": "+".join(filters(obj)),
           "n": colours(obj), "action": "keep"}
    try:
        im = decode(obj)
    except Exception as exc:  # noqa: BLE001
        rec["action"] = f"skip: decode {type(exc).__name__}: {exc}"[:120]
        return rec, None
    # largest placement decides how many pixels are needed
    pno, w_pt, h_pt, (pw, ph) = max(pl, key=lambda x: x[1] * x[2])
    rec["page"] = pno
    dpi_x = w / max(w_pt / 72, 1e-6)
    dpi_y = h / max(h_pt / 72, 1e-6)
    rec["dpi"] = round(min(dpi_x, dpi_y))
    page_long = max(pw, ph) / 72
    cover = (w_pt * h_pt) / max(pw * ph, 1)
    rec["cover"] = round(min(cover, 9.99), 3)
    new_w, new_h = w, h
    tgt = P.get("target_dpi")
    if tgt and 4.5 <= page_long <= 30 and min(dpi_x, dpi_y) > tgt * (1 + P["slack"]):
        new_w = max(1, round(w_pt / 72 * tgt))
        new_h = max(1, round(h_pt / 72 * tgt))
        if cover >= 0.5 and max(new_w, new_h) < P["min_long_px"]:
            f = min(P["min_long_px"] / max(new_w, new_h), max(w / new_w, h / new_h))
            new_w, new_h = round(new_w * f), round(new_h * f)
        new_w, new_h = min(new_w, w), min(new_h, h)
    resized = (new_w, new_h) != (w, h)
    grey = ""
    if P.get("gray") and im.mode == "RGB":
        grey = grey_kind(im, P["gray_tol"])
        if grey == "tint" and not P.get("gray_tint"):
            grey = ""
        if grey:
            im = im.convert("L")
    f = filters(obj)
    jpx = f == ["/JPXDecode"]
    big = cover >= P.get("reencode_min_cover", 0.0)
    reencode = resized or bool(grey) or (big and (jpx or (P.get("requality") and "/DCTDecode" in f) or
                                                  (P.get("flate_to_jpeg") and "/DCTDecode" not in f)))
    # the colour space written with a new stream: grey -> DeviceGray; JPX without one -> by mode
    new_cs = "/DeviceGray" if grey else (("/DeviceGray" if im.mode == "L" else "/DeviceRGB")
                                         if jpx and obj.get("/ColorSpace") is None else None)
    if P.get("bilevel_auto") and im.mode == "L" and cover >= 0.5:
        # an 8-bit grey page image that is in fact (nearly) black and white - a posterized scan stored
        # as 8-bit: thresholding it changes almost nothing (mean change <= bilevel_max_mae levels), so it
        # becomes a 1-bit image at its own resolution, stored as lossless JBIG2
        a = np.asarray(im)
        t = otsu(a)
        mae = float(np.abs(a.astype(np.int16) - np.where(a > t, 255, 0)).mean())
        ext = float(((a <= 48) | (a >= 208)).mean())
        rec["bilevel_mae"], rec["bilevel_ext"] = round(mae, 2), round(ext, 4)
        if mae <= P.get("bilevel_max_mae", 3.0) and ext >= P.get("bilevel_min_ext", 0.985) and text_like(im):
            bw = Image.fromarray(a > t).convert("1")
            data = jbig2_bytes(bw, f"{og[0]}_{og[1]}")
            rec.update(new=len(data), new_w=bw.width, new_h=bw.height, seconds=round(time.time() - t0, 2))
            if len(data) <= (1 - P["min_gain"]) * old:
                rec["action"] = "bilevel"
                return rec, {"data": data, "filter": "/JBIG2Decode", "w": bw.width, "h": bw.height,
                             "bpc": 1, "cs": "/DeviceGray"}
            rec["action"] = "keep (gain too small: bilevel)"
            return rec, None
    if P.get("pack_levels") and im.mode == "L" and not grey and not jpx \
            and "/DCTDecode" not in f and colours(obj) == 1:
        # a lossless 8-bit grey image that uses at most 16 grey levels (a posterized scan): the same pixels
        # as palette indices at 1, 2 or 4 bits (Indexed DeviceGray) - exactly lossless; 1-bit ones become
        # JBIG2 in stage 2
        a = np.asarray(im)
        lv = np.flatnonzero(np.bincount(a.ravel(), minlength=256))
        rec["levels"] = int(len(lv))
        if len(lv) <= 16:
            bits = 1 if len(lv) <= 2 else 2 if len(lv) <= 4 else 4
            lut = np.zeros(256, np.uint8)
            lut[lv] = np.arange(len(lv), dtype=np.uint8)
            data = zlib.compress(pack_bits(lut[a], bits), 9)
            rec.update(new=len(data), new_w=w, new_h=h, seconds=round(time.time() - t0, 2))
            if len(data) <= (1 - P["min_gain"]) * old:
                rec["action"] = f"pack{bits}"
                cs = "/DeviceGray" if list(lv) == [0, 255] else {"indexed_gray": [int(x) for x in lv]}
                return rec, {"data": data, "filter": "/FlateDecode", "w": w, "h": h, "bpc": bits, "cs": cs}
            rec["action"] = f"keep (gain too small: pack{bits})"
            return rec, None
    if P.get("bilevel") and im.mode == "L" and cover >= 0.5 and text_like(im):
        src = im.resize((new_w, new_h), Image.Resampling.LANCZOS) if resized else im
        if max(dpi_x, dpi_y) * (new_w / w) < 400:
            src = src.resize((src.width * 2, src.height * 2), Image.Resampling.BICUBIC)
        a = np.asarray(src)
        bw = Image.fromarray(a > otsu(a)).convert("1")
        data = jbig2_bytes(bw, f"{og[0]}_{og[1]}")
        rec.update(action="bilevel", new=len(data), new_w=bw.width, new_h=bw.height,
                   seconds=round(time.time() - t0, 2))
        if len(data) <= (1 - P["min_gain"]) * old:
            return rec, {"data": data, "filter": "/JBIG2Decode", "w": bw.width, "h": bw.height,
                         "bpc": 1, "cs": "/DeviceGray"}
        rec["action"] = "keep (bilevel gain too small)"
        return rec, None
    if not reencode:
        return rec, None
    if resized:
        im = im.resize((new_w, new_h), Image.Resampling.LANCZOS)
    buf = io.BytesIO()
    im.save(buf, "JPEG", quality=P["jpeg_q"], optimize=True)
    data = buf.getvalue()
    acts = [a for a, on in (("resize", resized), (grey or "grey", bool(grey)), ("jpx", jpx)) if on] or ["jpeg"]
    rec.update(new=len(data), new_w=im.width, new_h=im.height, seconds=round(time.time() - t0, 2))
    if len(data) <= (1 - P["min_gain"]) * old:
        rec["action"] = "+".join(acts)
        return rec, {"data": data, "filter": "/DCTDecode", "w": im.width, "h": im.height, "bpc": 8,
                     "cs": new_cs}
    rec["action"] = "keep (gain too small: " + "+".join(acts) + ")"
    return rec, None


# ------------------------------------------------------------------------------------------
# checks
# ------------------------------------------------------------------------------------------

def box(x: np.ndarray, k: int = 7) -> np.ndarray:
    """Mean over k x k windows ('valid'); sums in float64 (exact), result in float32."""
    h, w = x.shape
    c = np.zeros((h + 1, w), dtype=np.float64)
    np.cumsum(x, axis=0, dtype=np.float64, out=c[1:])
    r = c[k:] - c[:-k]
    del c
    c2 = np.zeros((r.shape[0], w + 1), dtype=np.float64)
    np.cumsum(r, axis=1, out=c2[:, 1:])
    del r
    return ((c2[:, k:] - c2[:, :-k]) * (1.0 / (k * k))).astype(np.float32)


def ssim(a: np.ndarray, b: np.ndarray, k: int = 7, strip: int = 384) -> tuple[float, float]:
    """Mean SSIM (7x7 uniform windows) over the page and over windows that hold ink; computed in
    horizontal strips (overlapping by k-1 rows) so memory stays small on large renders."""
    c1, c2 = (0.01 * 255) ** 2, (0.03 * 255) ** 2
    tot = n = tot_ink = n_ink = 0.0
    h = a.shape[0]
    for y0 in range(0, max(h - k + 1, 1), strip):
        y1 = min(h, y0 + strip + k - 1)
        if y1 - y0 < k:
            break
        sa = a[y0:y1].astype(np.float32)
        sb = b[y0:y1].astype(np.float32)
        ma, mb = box(sa), box(sb)
        saa = box(sa * sa) - ma * ma
        sbb = box(sb * sb) - mb * mb
        sab = box(sa * sb) - ma * mb
        s = ((2 * ma * mb + c1) * (2 * sab + c2)) / ((ma * ma + mb * mb + c1) * (saa + sbb + c2))
        ink = (saa > 64) | (ma < 200)
        tot += float(s.sum(dtype=np.float64))
        n += s.size
        tot_ink += float(s[ink].sum(dtype=np.float64))
        n_ink += float(ink.sum())
    return (tot / n if n else 1.0), (tot_ink / n_ink if n_ink else 1.0)


def blur(x: np.ndarray, k: int) -> np.ndarray:
    """k x k box blur, same size (edges padded)."""
    p = k // 2
    return np.clip(box(np.pad(x.astype(np.float32), ((p, p), (p, p)), mode="edge"), k), 0, 255).astype(np.uint8)


def ssim_structure(a: np.ndarray, b: np.ndarray, k: int) -> float:
    """Ink-SSIM after a small blur of both renders: blind to halftone screens and paper grain (which
    resampling smooths away), still very low for a missing, garbled, shifted or inverted image."""
    return ssim(blur(a, k), blur(b, k))[1]


def psnr(a: np.ndarray, b: np.ndarray) -> float:
    d = a.astype(np.float32) - b.astype(np.float32)
    mse = float((d * d).mean())
    return 99.0 if mse == 0 else round(10 * math.log10(255 ** 2 / mse), 2)


def render(page, scale: float) -> np.ndarray:
    bm = page.render(scale=scale, grayscale=True, may_draw_forms=False)
    a = bm.to_numpy()
    if a.ndim == 3:
        a = a[..., 0]
    return np.array(a, dtype=np.uint8)


def crop_box(a: np.ndarray, cw: int, ch: int) -> tuple[int, int]:
    h, w = a.shape
    if h <= ch or w <= cw:
        return 0, 0
    ink = 255 - a.astype(np.float32)
    best, by, bx = -1.0, 0, 0
    for y in range(0, h - ch, max(ch // 3, 1)):
        for x in range(0, w - cw, max(cw // 3, 1)):
            s = float(ink[y:y + ch, x:x + cw].mean())
            if s > best:
                best, by, bx = s, y, x
    return by, bx


def checks(n_pages: int, metric_pages: list[int], report: dict) -> None:
    import pypdfium2 as pdfium

    a = pdfium.PdfDocument(str(IN))
    b = pdfium.PdfDocument(str(W / "out.pdf"))
    report["pages_in"], report["pages_out"] = len(a), len(b)
    same, diff = 0, []
    def page_text(doc, i: int) -> str:
        # pdfium keeps every loaded page (and its decoded images) until it is closed explicitly
        pg = doc[i]
        tp = pg.get_textpage()
        t = tp.get_text_range()
        tp.close()
        pg.close()
        return t

    if len(a) == len(b):
        for i in range(len(a)):
            if page_text(a, i) == page_text(b, i):
                same += 1
            elif len(diff) < 20:
                diff.append(i)
    report["text_pages_identical"] = same
    report["text_pages_differ"] = diff
    kw, kh = P["kobo"]
    mets = []
    if P.get("crops"):
        (W / "crops").mkdir(exist_ok=True)
    for i in metric_pages:
        if i >= min(len(a), len(b)):
            continue
        pa, pb = a[i], b[i]
        wpt, hpt = pa.get_size()
        s = min(kw / wpt, kh / hpt)
        rec = {"page": i}
        for tag, sc in (("kobo", s), ("zoom2", 2 * s)):
            ra, rb = render(pa, sc), render(pb, sc)
            if ra.shape != rb.shape:
                rec[tag] = {"shape": [list(ra.shape), list(rb.shape)]}
                del ra, rb
                continue
            sg, si = ssim(ra, rb)
            rec[tag] = {"psnr": psnr(ra, rb), "ssim": round(sg, 4), "ssim_ink": round(si, 4)}
            if P.get("crops") and i in (P.get("crop_pages") or metric_pages[:3]):
                if tag == "zoom2":
                    y, x = crop_box(ra, 900, 600)
                    Image.fromarray(ra[y:y + 600, x:x + 900]).save(W / "crops" / f"p{i:04d}_zoom2_in.png")
                    Image.fromarray(rb[y:y + 600, x:x + 900]).save(W / "crops" / f"p{i:04d}_zoom2_out.png")
                else:
                    Image.fromarray(rb).save(W / "crops" / f"p{i:04d}_kobo_out.png")
                    Image.fromarray(ra).save(W / "crops" / f"p{i:04d}_kobo_in.png")
            del ra, rb
        pa.close()
        pb.close()
        mets.append(rec)
    report["metrics"] = mets
    vals = [m[t]["ssim_ink"] for m in mets for t in ("kobo", "zoom2") if "ssim_ink" in m.get(t, {})]
    report["ssim_ink_min_kobo"] = min((m["kobo"]["ssim_ink"] for m in mets if "ssim_ink" in m.get("kobo", {})), default=None)
    report["ssim_ink_min_zoom2"] = min((m["zoom2"]["ssim_ink"] for m in mets if "ssim_ink" in m.get("zoom2", {})), default=None)
    report["psnr_min_kobo"] = min((m["kobo"]["psnr"] for m in mets if "psnr" in m.get("kobo", {})), default=None)
    report["psnr_min_zoom2"] = min((m["zoom2"]["psnr"] for m in mets if "psnr" in m.get("zoom2", {})), default=None)
    report["metric_values"] = len(vals)
    a.close()
    b.close()


# ------------------------------------------------------------------------------------------

def main() -> int:
    global P
    import shutil
    if os.environ.get("MALLOC_MMAP_THRESHOLD_") is None:
        # large image buffers always mmapped, so freed memory goes back at once (keeps the peak under the
        # container's 900 MB cap on big JPEG 2000 pages); re-exec with the setting in place
        os.environ["MALLOC_MMAP_THRESHOLD_"] = "1048576"
        os.execv(sys.executable, [sys.executable] + sys.argv)
    t0 = time.time()
    P = dict(DEFAULTS)
    P.update(json.loads((W / "params.json").read_text()))
    report: dict = {"params": P, "bytes_in": IN.stat().st_size}
    pdf = pikepdf.open(IN)
    n_pages = len(pdf.pages)
    pl = placements(pdf)
    smasks = set()
    for og in pl:
        o = pdf.get_object(og)
        sm = o.get("/SMask")
        if sm is not None:
            smasks.add(sm.objgen)
    todo, skipped = [], {}
    for og, plist in pl.items():
        o = pdf.get_object(og)
        why = eligible(o, smasks)
        if why is None:
            todo.append((og, plist))
        else:
            skipped[why] = skipped.get(why, 0) + 1
    report["images_placed"] = len(pl)
    report["images_eligible"] = len(todo)
    report["images_skipped"] = skipped
    recs, changed_pages = [], {}
    stage1 = W / "stage1.pdf"
    changed = 0
    if todo and (P.get("target_dpi") or P.get("gray") or P.get("requality") or P.get("flate_to_jpeg")
                 or P.get("bilevel") or P.get("bilevel_auto") or P.get("pack_levels") or P.get("jpx")):
        if int(P.get("jobs") or 1) <= 1:
            init_worker(P)
            results = map(work, todo)
            ex = None
        else:
            ex = ProcessPoolExecutor(max_workers=P["jobs"], initializer=init_worker, initargs=(P,),
                                     mp_context=mp.get_context("fork"))
            results = ex.map(work, todo, chunksize=1)
        for rec, new in results:
            recs.append(rec)
            if new is None:
                continue
            o = pdf.get_object(tuple(rec["objgen"]))
            o.write(new["data"], filter=Name(new["filter"]))
            if "/DecodeParms" in o:
                del o["/DecodeParms"]
            o.Width, o.Height, o.BitsPerComponent = new["w"], new["h"], new["bpc"]
            if isinstance(new["cs"], dict):
                lv = new["cs"]["indexed_gray"]
                o.ColorSpace = pikepdf.Array([Name.Indexed, Name.DeviceGray, len(lv) - 1, pikepdf.String(bytes(lv))])
            elif new["cs"] is not None:
                o.ColorSpace = Name(new["cs"])
            saved = rec["old"] - len(new["data"])
            changed_pages[rec["page"]] = changed_pages.get(rec["page"], 0) + saved
            changed += 1
        if ex is not None:
            ex.shutdown()
    report["rss_mb_after_images"] = round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024)
    if changed:
        pdf.save(stage1)
    else:
        stage1 = IN
    pdf.close()
    report["rss_mb_after_stage1"] = round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024)
    report["stage1_bytes"] = stage1.stat().st_size
    report["images"] = recs
    acts: dict = {}
    for r in recs:
        k = r["action"].split(" (")[0] if r["action"].startswith("keep") else r["action"]
        a = acts.setdefault(k, {"n": 0, "old": 0, "new": 0})
        a["n"] += 1
        a["old"] += r["old"]
        a["new"] += r.get("new", r["old"]) if not r["action"].startswith("keep") else r["old"]
    report["actions"] = acts
    t1 = time.time()
    level = str(P.get("optimize") or 0)
    out = W / "out.pdf"
    out.unlink(missing_ok=True)
    if level != "0":
        if level in ("2nj", "2njnd"):
            # level 2 (pngquant for lossless images, JBIG2, Flate over JPEGs) but without its blanket
            # JPEG re-encoding at q75: JPEGs were already handled in stage 1 with a minimum-gain rule;
            # "2njnd" also leaves JPEGs without the extra Flate layer (MuPDF then decodes them faster)
            code = ("import sys, ocrmypdf.optimize as o; o._should_optimize_jpeg = lambda *a, **k: False; "
                    + ("o.deflate_jpegs = lambda *a, **k: None; " if level == "2njnd" else "")
                    + "o.main(sys.argv[1], sys.argv[2], 2)")
            cmd = ["nice", "-n", "19", sys.executable, "-c", code, str(stage1), str(out)]
        else:
            cmd = ["nice", "-n", "19", sys.executable, "-m", "ocrmypdf.optimize", str(stage1), str(out), level]
        r = subprocess.run(cmd, capture_output=True, text=True)
        report["optimize_rc"] = r.returncode
        if r.returncode or not out.exists():
            report["optimize_err"] = (r.stderr or "")[-800:]
            out.unlink(missing_ok=True)
    if not out.exists():
        if stage1 == IN:
            shutil.copyfile(IN, out)
        else:
            os.replace(stage1, out)
    if stage1 != IN and stage1.exists():
        stage1.unlink()
    report["optimize_seconds"] = round(time.time() - t1, 1)
    report["bytes_out"] = out.stat().st_size
    report["ratio"] = round(report["bytes_out"] / report["bytes_in"], 4)
    # pages to measure: given ones, else evenly spread + the pages that changed most
    mpages = list(P.get("metric_pages") or [])
    if not mpages:
        k = int(P.get("metric_auto") or 0)
        if k:
            spread = [int(n_pages * (i + 0.5) / (k // 2 or 1)) for i in range(k // 2 or 1)]
            top = [p for p, _ in sorted(changed_pages.items(), key=lambda kv: -kv[1])][: k - len(spread)]
            mpages = sorted(set(spread + top))
    report["changed_pages_top"] = [p for p, _ in sorted(changed_pages.items(), key=lambda kv: -kv[1])][:8]
    report["maxrss_mb_self"] = round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024)
    report["maxrss_mb_children"] = round(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss / 1024)
    report["stage_seconds"] = round(time.time() - t0, 1)
    (W / "report.json").write_text(json.dumps(report))
    # the checks run in a fresh process (exec): this one's heap (pikepdf objects, image buffers) is
    # released first, so pdfium's JPEG 2000 decoding has the whole memory cap
    os.execv(sys.executable, [sys.executable, sys.argv[0], "--checks", json.dumps(mpages), str(n_pages)])
    return 0


def main_checks() -> int:
    global P
    t2 = time.time()
    P = dict(DEFAULTS)
    P.update(json.loads((W / "params.json").read_text()))
    report = json.loads((W / "report.json").read_text())
    mpages, n_pages = json.loads(sys.argv[2]), int(sys.argv[3])
    checks(n_pages, mpages, report)
    report["check_seconds"] = round(time.time() - t2, 1)
    report["seconds"] = round(report.get("stage_seconds", 0) + report["check_seconds"], 1)
    report["maxrss_mb_checks"] = round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024)
    report["checks_done"] = True
    (W / "report.json").write_text(json.dumps(report))
    return 0


if __name__ == "__main__":
    sys.exit(main_checks() if len(sys.argv) > 1 and sys.argv[1] == "--checks" else main())
