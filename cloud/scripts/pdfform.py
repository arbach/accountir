#!/usr/local/lib/accountir/venv/bin/python
"""PDF form helper for accountir tax filing.

Usage:
  pdfform.py list <pdf>                 -> JSON {fields: [{name, type, value, states}]}
  pdfform.py fill <src> <dst> <json>    -> fills AcroForm fields from a JSON object
                                           {name: value}; checkbox values use the
                                           field's "on" state or true/false.
  pdfform.py text2png <out.png> <json>  -> render a typed signature to a tight,
                                           transparent PNG. json: {text, font, height?}
                                           font is a key under FONTS_DIR (e.g. GreatVibes).
  pdfform.py stamp <src> <dst> <json>   -> overlay signature image(s) + text onto pages.
                                           json: {stamps:[{page,x,y,w,image}],
                                                  texts:[{page,x,y,text,size?}]}
                                           coordinates are PDF points, origin bottom-left.
"""
import json
import os
import sys

from pypdf import PdfReader, PdfWriter

FONTS_DIR = os.environ.get("FONTS_DIR", "/usr/local/lib/accountir/fonts")


def font_path(key):
    # Sanitize: only a bare font key, mapped to <FONTS_DIR>/<key>.ttf
    safe = "".join(c for c in (key or "") if c.isalnum())
    p = os.path.join(FONTS_DIR, safe + ".ttf")
    if not safe or not os.path.exists(p):
        raise ValueError(f"unknown font '{key}'")
    return p


def list_fields(path):
    reader = PdfReader(path)
    out = []
    fields = reader.get_fields() or {}
    for name, f in fields.items():
        ft = str(f.get("/FT", ""))
        states = f.get("/_States_")
        out.append({
            "name": name,
            "type": {"/Tx": "text", "/Btn": "checkbox", "/Ch": "choice"}.get(ft, ft),
            "value": str(f.get("/V", "")) if f.get("/V") is not None else "",
            "states": [str(s) for s in states] if states else None,
        })
    print(json.dumps({"fields": out, "pages": len(reader.pages)}))


def fill(src, dst, fields_json):
    with open(fields_json) as fh:
        values = json.load(fh)
    reader = PdfReader(src)
    writer = PdfWriter()
    writer.append(reader)

    known = reader.get_fields() or {}
    coerced = {}
    for name, val in values.items():
        f = known.get(name)
        if f is not None and str(f.get("/FT", "")) == "/Btn":
            states = [str(s) for s in (f.get("/_States_") or []) if str(s) != "/Off"]
            on = states[0] if states else "/Yes"
            truthy = val in (True, "true", "True", "1", "Yes", "yes", "X", "x") or val == on
            coerced[name] = on.lstrip("/") if truthy else "/Off"
        else:
            coerced[name] = "" if val is None else str(val)

    for page in writer.pages:
        writer.update_page_form_field_values(page, coerced)
    try:
        writer.set_need_appearances_writer(True)
    except Exception:
        pass
    with open(dst, "wb") as fh:
        writer.write(fh)
    print(json.dumps({"ok": True, "written": dst, "applied": len(coerced)}))


def text2png(out_path, spec_json):
    from PIL import Image, ImageDraw, ImageFont
    with open(spec_json) as fh:
        spec = json.load(fh)
    text = (spec.get("text") or "").strip()
    if not text:
        raise ValueError("empty signature text")
    height = int(spec.get("height", 160))
    fnt = ImageFont.truetype(font_path(spec.get("font")), height)
    # Measure with a scratch canvas, then render onto a tight transparent bitmap.
    scratch = ImageDraw.Draw(Image.new("RGBA", (1, 1)))
    l, t, r, b = scratch.textbbox((0, 0), text, font=fnt)
    pad = max(8, height // 8)
    w, h = (r - l) + 2 * pad, (b - t) + 2 * pad
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw.text((pad - l, pad - t), text, font=fnt, fill=(15, 23, 42, 255))
    img.save(out_path, "PNG")
    print(json.dumps({"ok": True, "written": out_path, "w": w, "h": h}))


def stamp(src, dst, spec_json):
    import io
    from reportlab.pdfgen import canvas
    from reportlab.lib.utils import ImageReader
    with open(spec_json) as fh:
        spec = json.load(fh)
    stamps = spec.get("stamps", [])
    texts = spec.get("texts", [])
    reader = PdfReader(src)
    writer = PdfWriter()
    writer.append(reader)

    # Group overlay content by page index.
    pages = {}
    for s in stamps:
        pages.setdefault(int(s.get("page", 0)), {"stamps": [], "texts": []})["stamps"].append(s)
    for t in texts:
        pages.setdefault(int(t.get("page", 0)), {"stamps": [], "texts": []})["texts"].append(t)

    applied = 0
    for pidx, content in pages.items():
        if pidx < 0 or pidx >= len(writer.pages):
            continue
        page = writer.pages[pidx]
        box = page.mediabox
        pw, ph = float(box.width), float(box.height)
        buf = io.BytesIO()
        c = canvas.Canvas(buf, pagesize=(pw, ph))
        for s in content["stamps"]:
            img = ImageReader(s["image"])
            iw, ih = img.getSize()
            w = float(s.get("w", 160))
            h = w * ih / iw
            c.drawImage(img, float(s.get("x", 0)), float(s.get("y", 0)),
                        width=w, height=h, mask="auto")
            applied += 1
        for t in content["texts"]:
            c.setFont("Helvetica", float(t.get("size", 10)))
            c.setFillColorRGB(0.06, 0.09, 0.16)
            c.drawString(float(t.get("x", 0)), float(t.get("y", 0)), str(t.get("text", "")))
            applied += 1
        c.save()
        buf.seek(0)
        overlay = PdfReader(buf).pages[0]
        page.merge_page(overlay)

    with open(dst, "wb") as fh:
        writer.write(fh)
    print(json.dumps({"ok": True, "written": dst, "applied": applied}))


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    try:
        if cmd == "list" and len(sys.argv) == 3:
            list_fields(sys.argv[2])
        elif cmd == "fill" and len(sys.argv) == 5:
            fill(sys.argv[2], sys.argv[3], sys.argv[4])
        elif cmd == "text2png" and len(sys.argv) == 4:
            text2png(sys.argv[2], sys.argv[3])
        elif cmd == "stamp" and len(sys.argv) == 5:
            stamp(sys.argv[2], sys.argv[3], sys.argv[4])
        else:
            print(json.dumps({"error": "usage: pdfform.py list|fill|text2png|stamp ..."}))
            sys.exit(2)
    except Exception as e:  # surface as JSON for the Rust caller
        print(json.dumps({"error": f"{type(e).__name__}: {e}"}))
        sys.exit(1)
