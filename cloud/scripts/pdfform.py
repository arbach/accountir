#!/usr/local/lib/accountir/venv/bin/python
"""PDF form helper for accountir tax filing.

Usage:
  pdfform.py list <pdf>                 -> JSON {pages, sizes:[[w,h]…],
                                           fields:[{name,type,value,states,page,rect}]}
                                           rect = [xf,yf,wf,hf] fractions of the page,
                                           top-left origin (for positioning web overlays).
  pdfform.py fill <src> <dst> <json>    -> fills AcroForm fields from {name: value}.
  pdfform.py text2png <out.png> <json>  -> render a typed signature to a transparent PNG.
  pdfform.py stamp <src> <dst> <json>   -> overlay images / text / rectangles onto pages.
       json: {stamps:[{page, image, x,y,w  | xf,yf,wf}],
              texts:[{page, text, size?, color?, x,y | xf,yf}],
              rects:[{page, xf,yf,wf,hf, color:[r,g,b], opacity?}]}
       Absolute coords (x,y,w) are PDF points, origin bottom-left.
       Fractional coords (xf,yf,wf,hf) are 0..1 of the page, origin TOP-left.
"""
import json
import os
import sys

from pypdf import PdfReader, PdfWriter

FONTS_DIR = os.environ.get("FONTS_DIR", "/usr/local/lib/accountir/fonts")


def font_path(key):
    safe = "".join(c for c in (key or "") if c.isalnum())
    p = os.path.join(FONTS_DIR, safe + ".ttf")
    if not safe or not os.path.exists(p):
        raise ValueError(f"unknown font '{key}'")
    return p


def _widget_rects(reader):
    """Map field name -> (page_index, [xf,yf,wf,hf]) from page widget annotations."""
    out = {}
    for pidx, page in enumerate(reader.pages):
        box = page.mediabox
        pw, ph = float(box.width), float(box.height)
        if pw <= 0 or ph <= 0:
            continue
        annots = page.get("/Annots")
        if not annots:
            continue
        for a in annots:
            try:
                w = a.get_object()
            except Exception:
                continue
            if str(w.get("/Subtype")) != "/Widget":
                continue
            # Fully-qualified field name = the /T values from the widget up through
            # its ancestors joined by '.', matching PdfReader.get_fields() keys.
            parts, node, depth = [], w, 0
            while node is not None and depth < 12:
                t = node.get("/T")
                if t is not None:
                    parts.append(str(t))
                parent = node.get("/Parent")
                node = parent.get_object() if parent is not None else None
                depth += 1
            name = ".".join(reversed(parts))
            rect = w.get("/Rect")
            if not name or not rect or name in out:
                continue
            llx, lly, urx, ury = [float(v) for v in rect]
            xf = min(llx, urx) / pw
            wf = abs(urx - llx) / pw
            yf = (ph - max(lly, ury)) / ph
            hf = abs(ury - lly) / ph
            out[name] = (pidx, [xf, yf, wf, hf])
    return out


def list_fields(path):
    reader = PdfReader(path)
    rects = _widget_rects(reader)
    out = []
    fields = reader.get_fields() or {}
    for name, f in fields.items():
        ft = str(f.get("/FT", ""))
        states = f.get("/_States_")
        page, rect = rects.get(name, (None, None))
        out.append({
            "name": name,
            "type": {"/Tx": "text", "/Btn": "checkbox", "/Ch": "choice"}.get(ft, ft),
            "value": str(f.get("/V", "")) if f.get("/V") is not None else "",
            "states": [str(s) for s in states] if states else None,
            "page": page,
            "rect": rect,
        })
    sizes = [[float(p.mediabox.width), float(p.mediabox.height)] for p in reader.pages]
    print(json.dumps({"fields": out, "pages": len(reader.pages), "sizes": sizes}))


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
    reader = PdfReader(src)
    writer = PdfWriter()
    writer.append(reader)

    pages = {}
    for kind in ("stamps", "texts", "rects"):
        for item in spec.get(kind, []):
            pages.setdefault(int(item.get("page", 0)), {"stamps": [], "texts": [], "rects": []})[kind].append(item)

    applied = 0
    for pidx, content in pages.items():
        if pidx < 0 or pidx >= len(writer.pages):
            continue
        page = writer.pages[pidx]
        box = page.mediabox
        pw, ph = float(box.width), float(box.height)
        buf = io.BytesIO()
        c = canvas.Canvas(buf, pagesize=(pw, ph))

        # Rectangles first (highlight / redaction) so text/images sit on top.
        for r in content["rects"]:
            x = float(r.get("xf", 0)) * pw
            w = float(r.get("wf", 0)) * pw
            h = float(r.get("hf", 0)) * ph
            y = ph - float(r.get("yf", 0)) * ph - h
            col = r.get("color", [1, 1, 0])
            c.saveState()
            try:
                c.setFillAlpha(float(r.get("opacity", 0.35)))
            except Exception:
                pass
            c.setFillColorRGB(float(col[0]), float(col[1]), float(col[2]))
            c.rect(x, y, w, h, fill=1, stroke=0)
            c.restoreState()
            applied += 1

        for s in content["stamps"]:
            img = ImageReader(s["image"])
            iw, ih = img.getSize()
            if "xf" in s:
                w = float(s.get("wf", 0.2)) * pw
                h = w * ih / iw
                x = float(s.get("xf", 0)) * pw
                y = ph - float(s.get("yf", 0)) * ph - h
            else:
                w = float(s.get("w", 160))
                h = w * ih / iw
                x, y = float(s.get("x", 0)), float(s.get("y", 0))
            c.drawImage(img, x, y, width=w, height=h, mask="auto")
            applied += 1

        for t in content["texts"]:
            size = float(t.get("size", 11))
            c.setFont("Helvetica", size)
            col = t.get("color", [0.06, 0.09, 0.16])
            c.setFillColorRGB(float(col[0]), float(col[1]), float(col[2]))
            if "xf" in t:
                x = float(t.get("xf", 0)) * pw
                y = ph - float(t.get("yf", 0)) * ph - size
            else:
                x, y = float(t.get("x", 0)), float(t.get("y", 0))
            c.drawString(x, y, str(t.get("text", "")))
            applied += 1

        c.save()
        buf.seek(0)
        page.merge_page(PdfReader(buf).pages[0])

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
    except Exception as e:
        print(json.dumps({"error": f"{type(e).__name__}: {e}"}))
        sys.exit(1)
