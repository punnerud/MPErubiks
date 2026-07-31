#!/usr/bin/env python3
"""Static server for the web bundle + scan-capture uploads.

Serves ~/rubiks-dist like http.server did, and accepts POST /upload with
JSON {ts, face_idx, rotation, source, cells:[{w,h,rgba_hex}x9], votes:[..]}.
Each upload lands in ~/rubiks-scans/<stamp>/ as meta.json + cell_<i>.png
(PNG written with stdlib zlib only — no PIL on a stock Mac).
"""
import http.server
import json
import os
import struct
import time
import zlib

DIST = os.path.expanduser("~/rubiks-dist")
SCANS = os.path.expanduser("~/rubiks-scans")


def write_png(path, width, height, rgba):
    def chunk(tag, data):
        block = tag + data
        return (
            struct.pack(">I", len(data))
            + block
            + struct.pack(">I", zlib.crc32(block) & 0xFFFFFFFF)
        )

    raw = b"".join(
        b"\x00" + rgba[y * width * 4 : (y + 1) * width * 4] for y in range(height)
    )
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 6))
        + chunk(b"IEND", b"")
    )
    with open(path, "wb") as f:
        f.write(png)


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=DIST, **kw)

    def do_POST(self):
        if self.path != "/upload":
            self.send_error(404)
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            body = json.loads(self.rfile.read(length))
            stamp = time.strftime("%Y%m%d-%H%M%S") + f"-{int(time.time()*1000)%1000:03d}"
            folder = os.path.join(SCANS, stamp)
            os.makedirs(folder, exist_ok=True)
            cells = body.pop("cells", [])
            for i, cell in enumerate(cells):
                rgba = bytes.fromhex(cell["rgba_hex"])
                write_png(
                    os.path.join(folder, f"cell_{i}.png"),
                    cell["w"],
                    cell["h"],
                    rgba,
                )
            with open(os.path.join(folder, "meta.json"), "w") as f:
                json.dump(body, f, indent=1)
            self.send_response(200)
            self.send_header("Content-Length", "2")
            self.end_headers()
            self.wfile.write(b"ok")
        except Exception as e:  # noqa: BLE001 - report, keep serving
            print("upload failed:", e, flush=True)
            self.send_error(500)

    def log_message(self, fmt, *args):
        if "/upload" in (args[0] if args else ""):
            print(self.address_string(), fmt % args, flush=True)


if __name__ == "__main__":
    os.makedirs(SCANS, exist_ok=True)
    http.server.ThreadingHTTPServer(("0.0.0.0", 8877), Handler).serve_forever()
