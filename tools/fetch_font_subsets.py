#!/usr/bin/env python3
"""Build per-language FONT SUBSETS for scripts the default UI font lacks.

The app's translations are short: a whole CJK translation uses a few
hundred distinct characters. Google's font API can serve a subset
containing exactly the characters you name (`text=`), and Noto is
OFL-licensed, so the result is redistributable. A full Noto CJK is
~10 MB; the subsets this produces are tens of KB, fetched only when
someone actually picks that language.

Usage: python3 tools/fetch_font_subsets.py [code ...]
Writes assets/fonts/<code>.ttf and prints the sizes.
"""
import csv
import os
import re
import sys
import urllib.parse
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Language code -> Google Fonts family covering its script.
FAMILIES = {
    "zh-Hans": "Noto Sans SC",
    "zh-Hant": "Noto Sans TC",
    "ja": "Noto Sans JP",
    "ko": "Noto Sans KR",
}

# A legacy UA makes the API answer with TTF instead of woff2 (egui's
# font stack reads TTF/OTF, not woff2).
UA_TTF = "Mozilla/4.0"


def chars_used(code: str) -> str:
    """Every distinct character in this language's translation, plus the
    digits and punctuation the UI composes at runtime."""
    path = os.path.join(ROOT, "assets", "i18n", f"{code}.csv")
    seen = set("0123456789.,:;!?()[]%+-–—…'\"/ ")
    with open(path, encoding="utf-8") as f:
        for row in csv.reader(f):
            if len(row) >= 2:
                seen.update(row[1])
    return "".join(sorted(seen))


def fetch_subset(family: str, text: str) -> bytes:
    css_url = (
        "https://fonts.googleapis.com/css2?family="
        + urllib.parse.quote(family.replace(" ", "+"), safe="+")
        + "&text="
        + urllib.parse.quote(text)
    )
    req = urllib.request.Request(css_url, headers={"User-Agent": UA_TTF})
    css = urllib.request.urlopen(req, timeout=60).read().decode()
    m = re.search(r"url\((https://[^)]+)\)", css)
    if not m:
        raise SystemExit(f"no font url in CSS for {family}:\n{css[:400]}")
    font_req = urllib.request.Request(m.group(1), headers={"User-Agent": UA_TTF})
    data = urllib.request.urlopen(font_req, timeout=120).read()
    if data[:4] not in (b"\x00\x01\x00\x00", b"true", b"OTTO"):
        raise SystemExit(f"{family}: not a TTF/OTF (got {data[:4]!r})")
    return data


def main() -> None:
    codes = sys.argv[1:] or list(FAMILIES)
    out_dir = os.path.join(ROOT, "assets", "fonts")
    os.makedirs(out_dir, exist_ok=True)
    total = 0
    for code in codes:
        family = FAMILIES[code]
        text = chars_used(code)
        data = fetch_subset(family, text)
        path = os.path.join(out_dir, f"{code}.ttf")
        with open(path, "wb") as f:
            f.write(data)
        total += len(data)
        print(f"{code:8s} {family:14s} {len(text):4d} chars -> {len(data):7d} B")
    print(f"total {total} B across {len(codes)} subsets")


if __name__ == "__main__":
    main()
