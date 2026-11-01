# Stump post-processing: the PDF host runner

Stump can save space after a scan (library settings → **Post-processing**):

- **Manga, 16 grey levels**: done inside Stump (the `post_processing` job, its own worker, idle
  priority). Same rules as book-curator's `src/comics_grey16.py`.
- **Scanned PDFs, OCR and shrinking**: done on the host by `pdf_runner.py`, with book-curator's
  recipe and code (`research/pdf-size.md`). OCRmyPDF, Tesseract, Ghostscript, JBIG2 and PyMuPDF
  stay out of Stump's image, and nothing runs while the queue is empty.

Stump never deletes a book: an original is copied to the quarantine folder (dated batch folders
with a `restore_log.csv`: `original_path,quarantine_path,bytes_before,bytes_after`, paths as the
container sees them) before the processed file replaces it. Without a quarantine folder Stump
replaces nothing.

## The queue

`<config>/workers/pdf/` (on this host: `/mnt/data2/containers/stump-fork/config/workers/pdf/`):

| Folder | Written by | Holds |
|---|---|---|
| `queue/<id>.json` | Stump | a request: container path, size and mtime when queued, steps (`ocr`, `shrink`), language hint |
| `running/<id>.json` | runner | the request being processed |
| `done/<id>.json` (+ `<id>.pdf`) | runner | the result: `done` (a new file), `not_worth`, `skipped` or `failed`, with a reason |
| `archive/<id>.json` | Stump | results Stump applied |

Stump looks at `done/` every minute. It swaps a new file in only when the library file still has
the size and mtime it had when it was queued; otherwise the task is marked *Kept* ("the library
file changed after it was queued").

What the runner does per PDF: classify the text layer (`map_pdfs.py`): born-digital PDFs are
skipped; OCR only when the text layer is missing, partial or poor (`--skip-text` or `--redo-ocr`,
`spa+eng` for Spanish books, `eng` otherwise); then the `auto200` size step, measured first on a
12-page excerpt, kept only under the research's rule (same pages and text, same outline,
structural SSIM ≥ 0.90 at Kobo size and ≥ 0.88 at 2×, saving ≥ 15 % and ≥ 2 MB). Each heavy step
is one job of book-curator's shared `g3slot` pool and waits for 3 GB of free memory. The GPU does
not help: Tesseract and the image re-encoding are CPU-bound.

## Install (once, on g3)

1. Mount a quarantine folder into Stump and tell Stump about it (docker-compose of `stump-fork`):

   ```yaml
       volumes:
         - /mnt/data2/Books/Biblioteca-quarantine:/quarantine:rw
       environment:
         - STUMP_QUARANTINE_DIR=/quarantine
   ```

2. Start the runner when Stump queues a PDF, with the systemd user units in this folder:

   ```bash
   mkdir -p ~/.config/systemd/user
   cp scripts/fork/workers/stump-pdf-runner.{service,path} ~/.config/systemd/user/
   systemctl --user daemon-reload
   systemctl --user enable --now stump-pdf-runner.path
   loginctl enable-linger "$USER"     # so it also runs while nobody is logged in
   ```

   Or, instead, the cron line in `stump-pdf-runner.cron` (every 10 minutes).

The runner reads the mounts of the `stump-fork` container (`docker inspect`) to map container
paths to host paths (`STUMP_PATH_MAP` overrides that), uses book-curator's virtualenv
(`/mnt/data2/Code/book-curator/.venv`) and works in `/mnt/nvme/Downloads/stump-pdf-runner/`.

```bash
/mnt/data2/Code/book-curator/.venv/bin/python scripts/fork/workers/pdf_runner.py status
journalctl --user -u stump-pdf-runner -f        # the runner's log (systemd)
python3 scripts/fork/workers/test_pdf_runner.py # queue protocol tests (fake jobs, no docker)
```
