#!/usr/bin/env python3
"""make-test-epub.py: a small, valid EPUB with things that must never reach the library.

usage: make-test-epub.py <out.epub>

The book has one page that holds a <script>, an event handler, an <iframe>, a javascript: link and a
form. It is the check of DEPLOY.md ("Enable friend uploads"): sent through the public edge by a friend,
the checker rebuilds it, and the copy that is filed holds none of those (the description in the package is
plain text). Nothing in it is harmful by itself; the script only sets a variable.
"""
import sys
import zipfile

PAGE = """<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Test page</title>
<script>window.__test_marker = 1;</script></head>
<body onload="window.__test_marker = 2">
<h1 onclick="window.__test_marker = 3">A hostile test book</h1>
<p>Plain text that must survive the rebuild.</p>
<p><a href="javascript:window.__test_marker = 4">a script link</a></p>
<iframe src="https://example.invalid/frame"></iframe>
<form action="https://example.invalid/post"><input name="x"/></form>
</body></html>
"""
NAV = """<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head>
<body><nav epub:type="toc"><ol><li><a href="page1.xhtml">A hostile test book</a></li></ol></nav></body></html>
"""
CONTAINER = """<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>
"""
OPF = """<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:identifier id="id">urn:uuid:7d4f6b0e-0000-4000-8000-000000000001</dc:identifier>
<dc:title>A hostile test book</dc:title><dc:creator>Test Author</dc:creator><dc:language>en</dc:language>
<dc:description>One short plain description for the deploy check.</dc:description>
<meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata>
<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
<item id="p1" href="page1.xhtml" media-type="application/xhtml+xml"/></manifest>
<spine><itemref idref="p1"/></spine></package>
"""


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    with zipfile.ZipFile(sys.argv[1], "w") as z:
        z.writestr(zipfile.ZipInfo("mimetype"), "application/epub+zip", compress_type=zipfile.ZIP_STORED)
        z.writestr("META-INF/container.xml", CONTAINER, compress_type=zipfile.ZIP_DEFLATED)
        z.writestr("OEBPS/content.opf", OPF, compress_type=zipfile.ZIP_DEFLATED)
        z.writestr("OEBPS/nav.xhtml", NAV, compress_type=zipfile.ZIP_DEFLATED)
        z.writestr("OEBPS/page1.xhtml", PAGE, compress_type=zipfile.ZIP_DEFLATED)
    print("wrote", sys.argv[1])


if __name__ == "__main__":
    main()
