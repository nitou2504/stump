# Stump post-processing: the PDF worker container

Stump can save space after a scan (library settings → **Post-processing**):

- **Manga, 16 grey levels**: done inside Stump (the `post_processing` job, its own worker, idle
  priority). Same rules as book-curator's `src/comics_grey16.py`.
- **Scanned PDFs, OCR and shrinking**: done by the **PDF worker**, a second container next to
  Stump (`docker/Dockerfile.pdf-worker`, built on `jbarlow83/ocrmypdf-ubuntu:v17.13.0`) with
  book-curator's recipe (`research/pdf-size.md`) vendored in `pdfrecipe/`. OCRmyPDF, Tesseract,
  Ghostscript, JBIG2 and PyMuPDF stay out of Stump's image, and nothing runs on the host.

Stump never deletes a book: an original is copied to the quarantine folder (dated batch folders
with a `restore_log.csv`: `original_path,quarantine_path,bytes_before,bytes_after`, paths as the
container sees them) before the processed file replaces it. Without a quarantine folder Stump
replaces nothing.

## Setup (docker compose only)

`docker/compose.fork-postproc.example.yml` has both services. In short:

1. Stump gets the quarantine folder: a volume such as
   `/mnt/data2/Books/Biblioteca-quarantine:/quarantine` and `STUMP_QUARANTINE_DIR=/quarantine`.
2. A `stump-pdf-worker` service built from this repository (`build: context: <stump checkout>,
   dockerfile: docker/Dockerfile.pdf-worker`) mounts Stump's `<config>/workers/pdf` folder
   read-write and the library folders **read-only, at the same paths as in Stump** (the requests
   carry Stump's paths), plus a named volume for its work files (`/work`). It runs as `1000:1000`,
   with `cpus: 1.5` and `mem_limit: 1536m`.
3. Create the queue folder once before the first `docker compose up`, so that it belongs to Stump's
   user (docker creates a missing bind-mount folder as root): `mkdir -p ./config/workers/pdf`.

`docker compose up -d --build` builds the worker image; `docker compose logs -f stump-pdf-worker`
shows what it does.

## How the worker runs

`pdf_worker.sh` (the container's entrypoint, bash) keeps one `inotifywait -m` on `queue/` for the
life of the container (so no request is missed while it is busy) and starts
`pdf_runner.py drain` only when Stump has queued something; drain works through the queue at
nice 19 and idle I/O priority, one PDF at a time (`PDF_WORKER_JOBS`), and exits. Waiting costs a
shell and inotifywait, a few MB. `PDF_WORKER_POLL=30` checks the folder every 30 s instead of
inotify (for file systems that don't deliver inotify events).

Every 5 minutes (`PDF_WORKER_HEARTBEAT`), and while it works, the worker writes `worker.json`
(version, idle/busy, the task it is on, last update). The post-processing settings page shows it
and says *PDF worker container not running* when the heartbeat is missing or older than three
intervals; "Queue PDFs" is disabled then.

Other knobs: `PDF_WORKER_MIN_FREE_MB` (default 1536: a heavy step waits while the host has less
memory available), a `PAUSE` file in the queue folder (stops after the current step and leaves the
rest queued), and book-curator's g3 slot pool when its folder is mounted at `/g3-heavy-slots`
(`- /tmp/g3-heavy-slots:/g3-heavy-slots`, only if that folder already exists).

## The queue

`<config>/workers/pdf/` (here: `/mnt/data2/containers/stump-fork/config/workers/pdf/`):

| Path | Written by | Holds |
|---|---|---|
| `queue/<id>.json` | Stump | a request: container path, size and mtime when queued, steps (`ocr`, `shrink`), language hint |
| `running/<id>.json` | worker | the request being processed |
| `done/<id>.json` (+ `<id>.pdf`) | worker | the result: `done` (a new file), `not_worth`, `skipped` or `failed`, with a reason |
| `archive/<id>.json` | Stump | results Stump applied |
| `worker.json` | worker | its heartbeat |

Stump looks at `done/` every minute. It swaps a new file in only when the library file still has
the size and mtime it had when it was queued; otherwise the task is marked *Kept* ("the library
file changed after it was queued").

What the worker does per PDF: classify the text layer (`map_pdfs.py`): born-digital PDFs are
skipped; OCR only when the text layer is missing, partial or poor (`--skip-text` or `--redo-ocr`,
`spa+eng` for Spanish books, `eng` otherwise); then the `auto200` size step, measured first on a
12-page excerpt, kept only under the research's rule (same pages and text, same outline,
structural SSIM ≥ 0.90 at Kobo size and ≥ 0.88 at 2×, saving ≥ 15 % and ≥ 2 MB). The GPU does not
help: Tesseract and the image re-encoding are CPU-bound.

## The vendored recipe

`pdfrecipe/` holds book-curator's code as it was on 2026-11-02 (HEAD 1968187): `map_pdfs.py` and
`shrink_worker.py` unchanged apart from a header, `recipe.py` with the parts of `shrink_queue.py`,
`shrink.py`, `make_plan.py` and `ocr_queue.py` the worker uses (copied as they are; the file lists
the sources and their hashes), and the wordfreq word lists. When book-curator changes its recipe,
copy the changes here.

```bash
docker compose exec stump-pdf-worker python3 /opt/stump-pdf-worker/pdf_runner.py status
docker run --rm --entrypoint python3 stump-pdf-worker:<rev> /opt/stump-pdf-worker/test_pdf_runner.py
python3 scripts/fork/workers/test_pdf_runner.py   # the same tests outside the image (fake jobs)
```
