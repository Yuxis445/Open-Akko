#!/usr/bin/env python3
"""Regenerate Open Akko's data/ tables from the official Akko Cloud v4 installer.

Usage:
    python3 -I tools/build_data.py "Akko Cloud_v4 Windows驱动程序.exe"   # or an extracted resources/app dir
    python3 -I tools/build_data.py <input> --out data

Needs: 7z (for the .exe) and the `jsbeautifier` Python package
(e.g. `python3 -m venv /tmp/v && /tmp/v/bin/pip install jsbeautifier` then run with /tmp/v/bin/python).

Produces data/devices.json, data/models.json, data/layouts.json.
Tested with Akko Cloud v4 371.2.19; chunk files are located by content, not by name.
"""
import argparse
import glob
import json
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))


def run(*args):
    subprocess.run(args, check=True)


def extract_app(src, tmp):
    """Return the resources/app directory for an .exe or an already extracted folder."""
    if os.path.isdir(src):
        for cand in (src, os.path.join(src, "resources", "app"), os.path.join(src, "app", "resources", "app")):
            if os.path.isdir(os.path.join(cand, "dist", "js")):
                return cand
        sys.exit(f"no dist/js found under {src}")
    if not shutil.which("7z"):
        sys.exit("7z is required to unpack the installer")
    nsis = os.path.join(tmp, "nsis")
    run("7z", "x", "-y", f"-o{nsis}", src)
    inner = glob.glob(os.path.join(nsis, "$PLUGINSDIR", "app-*.7z"))
    if not inner:
        sys.exit("installer layout not recognised (no $PLUGINSDIR/app-*.7z)")
    app = os.path.join(tmp, "app")
    run("7z", "x", "-y", f"-o{app}", inner[0])
    return os.path.join(app, "resources", "app")


def find_chunk(js_dir, needle):
    for f in sorted(glob.glob(os.path.join(js_dir, "*.js"))):
        with open(f, encoding="utf-8", errors="ignore") as fh:
            if needle in fh.read():
                return f
    sys.exit(f"no chunk containing {needle!r}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("input", help="Akko Cloud v4 installer .exe or extracted app folder")
    ap.add_argument("--out", default=os.path.join(HERE, "..", "data"))
    a = ap.parse_args()
    try:
        import jsbeautifier
    except ImportError:
        sys.exit("missing Python package 'jsbeautifier' (see the docstring)")

    with tempfile.TemporaryDirectory() as tmp:
        app = extract_app(os.path.abspath(a.input), tmp)
        js = os.path.join(app, "dist", "js")
        index = glob.glob(os.path.join(js, "index.*.js"))[0]
        pretty = os.path.join(tmp, "pretty")
        os.makedirs(pretty)
        opts = jsbeautifier.default_options()

        def beautify(src, name):
            out = os.path.join(pretty, name)
            with open(out, "w", encoding="utf-8") as fh:
                fh.write(jsbeautifier.beautify_file(src, opts))
            return out

        print("beautifying bundles…")
        beautify(index, "index.js")
        finder = beautify(find_chunk(js, "as findYC3121dev"), "finder.js")
        base = beautify(find_chunk(js, "as CommonKbYc500"), "base.js")

        py = [sys.executable, "-I"]
        all_dev = os.path.join(tmp, "devices_all.json")
        models = os.path.join(tmp, "models.json")
        layouts = os.path.join(tmp, "layouts.json")
        run(*py, os.path.join(HERE, "extract_devices.py"), index, all_dev)
        run(*py, os.path.join(HERE, "extract_models.py"), finder, base, models)

        devices = json.load(open(all_dev, encoding="utf-8"))
        akko = [d for d in devices if (d.get("company") or "").lower() == "akko"]
        wanted = sorted({d["keyLayout"] for d in akko if d.get("keyLayout")})
        run(*py, os.path.join(HERE, "extract_layouts.py"), js, pretty, layouts, ",".join(wanted))

        m = json.load(open(models, encoding="utf-8"))
        for d in akko:
            d["supported"] = d["name"] in m
        used = {d["name"] for d in akko} | {"_generic"}
        os.makedirs(a.out, exist_ok=True)
        json.dump(akko, open(os.path.join(a.out, "devices.json"), "w"))
        json.dump({k: v for k, v in m.items() if k in used}, open(os.path.join(a.out, "models.json"), "w"))
        shutil.copy(layouts, os.path.join(a.out, "layouts.json"))
        print(f"wrote {a.out}: {len(akko)} Akko devices, {sum(d['supported'] for d in akko)} supported")


if __name__ == "__main__":
    main()
