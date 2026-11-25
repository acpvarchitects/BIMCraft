"""Tables for step 2 (laptop): hard cases against ADE, and the panel for arris 0.12 and solidcore main.
usage: python -I tables-step2.py <bench checkout> <out.md>"""
import json, os, re, sys
R, OUT = sys.argv[1], sys.argv[2]
sys.path.insert(0, os.path.join(R, "harness")); import hardcases as H
W = os.path.join(R, "results", "workstation-emiliano")
NAMES = {"arris": "arris 0.12.0", "sc-main": "solidcore main", "cadcore": "CadCore main"}
md = []

# --- hard cases: the laptop table, then where the laptop's outcome differs from ADE's
paths = [os.path.join(W, "hardcases", f"{k}.jsonl") for k in NAMES]
tb = H.table([p for p in paths if os.path.exists(p)])
head, rest = tb.split("\n", 1)
for k, v in NAMES.items(): head = head.replace(f" {k} ", f" {v} ")
md += ["### Hard cases on the laptop", "",
       "Times are the kernel's own (`ms` in the case's last line), not the process wall. — = not run: CadCore runs only its 15 cases that have an operation (the other 28 are no op, as on ADE).", "",
       head + "\n" + rest, ""]
diff, same, speed = [], 0, {}
for k in NAMES:
    lap = {json.loads(l)["name"]: json.loads(l) for l in open(os.path.join(W, "hardcases", f"{k}.jsonl"))}
    ade = {json.loads(l)["name"]: json.loads(l) for l in open(os.path.join(R, "results", "ade-hardcases", f"{k}.jsonl"))}
    for n, r in lap.items():
        a = ade.get(n)
        if not a: continue
        sl, nl = H.classify(r); sa, na = H.classify(a)
        if sl != sa: diff.append(f"- {n}, {NAMES[k]}: laptop **{sl}** ({nl}), ADE **{sa}** ({na})")
        else: same += 1
        def ms(x):
            try: return json.loads(x["stdout"].splitlines()[-1]).get("ms")
            except Exception: return None
        if sl == sa == "ok" and ms(r) and ms(a) and min(ms(r), ms(a)) >= 5: speed.setdefault(k, []).append((n, ms(r), ms(a)))
md += ["### Laptop against ADE", "", f"{same} case runs have the same outcome on both machines." + (" These differ:" if diff else " None differs."), ""] + diff + [""]
for k in NAMES:
    xs = speed.get(k)
    if not xs: md.append(f"- {NAMES[k]}: no case that both ran ok takes 5 ms or more, so no ratio."); continue
    import statistics
    ratio = statistics.median(a / l for _, l, a in xs if l > 0)
    slow = sorted(xs, key=lambda x: -x[1])[:3]
    md.append(f"- {NAMES[k]}: on the {len(xs)} cases both ran ok in 5 ms or more, the laptop is {ratio:.2f}x faster than ADE (median of ADE time / laptop time); slowest here: "
              + "; ".join(f"{n} {l:.0f} ms (ADE {a:.0f})" for n, l, a in slow))
md.append("")

# --- panel
def parse(out):
    d = {}
    m = re.search(r"PANELS .*?build_ok=(\d+) errors=(\d+).*?build_wall_ms=(\d+)(?:.*first_error=(\".*\"))?", out)
    if m: d.update(ok=int(m[1]), errors=int(m[2]), build=int(m[3]) / 1e3, first=(m[4] or "").strip('"'))
    for c, tris, per, wall in re.findall(r"MESH chord=([\d.]+)mm tris=(\d+) per_panel=(\d+)(?: failed=\d+)? wall_ms=(\d+)", out):
        d[c] = (int(per), int(wall) / 1e3)
    fails = re.findall(r"MESH chord=([\d.]+)mm .*?failed=(\d+)", out)
    d["mesh_failed"] = {c: int(f) for c, f in fails if int(f)}
    return d
for k in ("arris", "sc-main"):
    p = os.path.join(W, "panels", f"{k}.jsonl")
    if not os.path.exists(p): continue
    md += [f"### Panel: {NAMES[k]}, mimalloc", "",
           "| call | variant | panels | threads | build wall | mesh 2 mm (tris/panel, wall) | mesh 0.5 mm (tris/panel, wall) | peak RSS | CPU | process wall | status |",
           "|---|---|---|---|---|---|---|---|---|---|---|"]
    notes = []
    for l in open(p):
        r = json.loads(l); d = parse(r["stdout"]); v, n, t = r["name"].split("-")[0], r["name"].split("-")[1], r["threads"]
        f = lambda c: f"{d[c][0]}, {d[c][1]:.2f} s" if c in d else "—"
        b = f"{d['build']:.2f} s" if "build" in d else "—"
        if d.get("errors"): b += f" ({d['errors']} failed)"; notes.append(f"- {r['name']}: {d['errors']} panels failed; first: {d.get('first')}")
        if d.get("mesh_failed"): notes.append(f"- {r['name']}: mesh failed on " + ", ".join(f"{x} panels at {c} mm" for c, x in d["mesh_failed"].items()))
        md.append(f"| {r['name']} | {v} | {n} | {t} | {b} | {f('2')} | {f('0.5')} | {r['rss_mb']} MB | {r.get('cpu_s')} s | {r['wall_s']:.1f} s | {r['status']} |")
    md += [""] + notes + [""]
open(OUT, "w", encoding="utf-8", newline="\n").write("\n".join(md))
cut = md.index("### Panel: arris 0.12.0, mimalloc") if "### Panel: arris 0.12.0, mimalloc" in md else len(md)
open(os.path.join(W, "hardcases", "table.md"), "w", encoding="utf-8", newline="\n").write(
    "# Hard cases on Emiliano's laptop (MSVC, mimalloc), 2026-11-25\n\nStep 2 in ../README.md. Records: `<kernel>.jsonl` here; logs: `../logs/hardcases/`.\n\n"
    + "\n".join(md[:cut]).rstrip() + "\n")
print("\n".join(md))
