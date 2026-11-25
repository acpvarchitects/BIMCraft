#!/usr/bin/env python3
import json, re, os, glob
root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
K = [("bimcraft", "BIMCraft mesh path (PR #81 geo_bench)"), ("arris", "arris 0.11.1"), ("solidcore", "solidcore 1.0.1"), ("fornjot", "Fornjot fj-core 0.49.0"), ("truck", "truck (modeling 0.6.0 / shapeops 0.4.0 / meshalgo 0.4.0)"), ("occt", "OCCT 7.8.1 via opencascade-rs 0.3.0")]
def load(k):
    d = {}
    p = f"{root}/results/{k}.jsonl"
    if os.path.exists(p):
        for l in open(p):
            r = json.loads(l); d[(r["group"], r["name"])] = r
    return d
R = {k: load(k) for k, _ in K}
def first(s): return (s.strip().splitlines() or [""])[0]
def cells(line):
    c = [x.strip() for x in line.strip().strip("|").split("|")]
    return c
def child(r):
    s = r["status"]; return f'{s}, {r["rss_mb"]} MB' if r["rss_mb"] is not None else s
def rel(r):
    """one-word outcome for the summary matrix"""
    o = first(r["stdout"]); st = r["status"]
    if st != "ok": return st.split("(")[0]
    c = cells(o); res = c[1] if len(c) > 1 else o
    if res.startswith("CAN'T") or res.lower().startswith("can't"): return "can't build"
    if res.startswith("not run"): return "not run"
    if res.startswith("PANIC"): return "PANIC"
    if res.startswith("ERR"): return "ERR"
    if "INVALID" in res: return "built, INVALID"
    return "ok"
def forms_table(k, group):
    out = []
    rows = [(n, r) for (g, n), r in R[k].items() if g == group]
    if not rows: return ""
    if group == "forms":
        out.append("| case | result | detail | op time | mesh 2 mm / 0.5 mm | child (status, peak RSS) |\n|---|---|---|---|---|---|")
        for n, r in rows:
            o = first(r["stdout"]); c = cells(o)
            if n == "STEP": out.append(f"| STEP | {o[:160]} | | | | {child(r)} |"); continue
            if r["status"] != "ok" and len(c) < 2: out.append(f"| {n} | {r['status']} | | | | {child(r)} |"); continue
            res = c[1] if len(c) > 1 else o
            mid = "; ".join(c[2:-2]) if len(c) > 4 else ""
            out.append(f"| {n} | {res} | {mid} | {c[-2] if len(c) > 3 else ''} | {c[-1] if len(c) > 3 else ''} | {child(r)} |")
    else:
        out.append("| case | result | time | child |\n|---|---|---|---|")
        for n, r in rows:
            c = cells(first(r["stdout"]))
            out.append(f"| {c[0] if c else n} | {c[1] if len(c) > 1 else r['status']} | {c[2] if len(c) > 2 else ''} | {child(r)} |")
    return "\n".join(out)
def pan(r):
    o = r["stdout"]; d = {}
    m = re.search(r"build_ok=(\d+) errors=(\d+)(?: panics=(\d+))? build_wall_ms=(\d+)", o) or re.search(r"build_ok=(\d+) errors=(\d+) build_wall_ms=(\d+)", o)
    b = re.search(r"build_ok=(\d+) errors=(\d+).*?build_wall_ms=(\d+)", o)
    if b: d.update(ok=int(b.group(1)), err=int(b.group(2)), wall=int(b.group(3)) / 1000)
    d["mesh"] = {m_.group(1): (int(m_.group(2)), int(m_.group(3)), float(m_.group(4))) for m_ in re.finditer(r"MESH chord=([\d.]+)mm tris=(\d+) per_panel=(\d+) wall_ms=(\d+)", o)} if False else {}
    for m_ in re.finditer(r"MESH chord=([\d.]+)mm tris=(\d+) per_panel=(\d+) wall_ms=(\d+)", o): d["mesh"][m_.group(1)] = (int(m_.group(2)), int(m_.group(3)), int(m_.group(4)) / 1000)
    pm = re.search(r"panics=(\d+)", o); d["panics"] = int(pm.group(1)) if pm else None
    return d
def fm(x): return f"{x:.2f}" if x < 10 else f"{x:.1f}" if x < 100 else f"{x:.0f}"
def panels_table(k):
    rows = sorted([(n, r) for (g, n), r in R[k].items() if g == "panels"], key=lambda x: (x[0].split("-")[0], -int(x[0].split("-")[1]), int(x[0].split("-t")[1])))
    if not rows: return ""
    if k == "bimcraft":
        out = ["| call | GEO_PANELS | threads | all-cores build of N panels (extrapolated to 10,000) no void | with void | per-panel 1 core (void) | tris/panel | child |", "|---|---|---|---|---|---|---|---|"]
        for n, r in rows:
            o = r["stdout"] + "\n" + r.get("stderr_tail", "")
            a = re.search(r"no void\s*: all cores ([\d.]+) s, one core ([\d.]+) ms a panel, 10 changed ([\d.]+) ms, (\d+) triangles", o)
            b = re.search(r"with a void: all cores ([\d.]+) s, one core ([\d.]+) ms a panel, 10 changed ([\d.]+) ms, (\d+) triangles", o)
            N = n.split("-")[1]; t = n.split("-t")[1]
            out.append(f"| {n} | {N} | {t} | {a.group(1) + ' s' if a else '?'} | {b.group(1) + ' s' if b else '?'} | {b.group(2) + ' ms' if b else '?'} | {a.group(4) if a else '?'} / {b.group(4) if b else '?'} | {child(r)} |")
        return "\n".join(out)
    out = ["| void | panels | threads | status | build wall (s) | built ok / errors / panics | mesh 2 mm (tris/panel, s) | mesh 0.5 mm (tris/panel, s) | peak RSS |", "|---|---|---|---|---|---|---|---|---|"]
    for n, r in rows:
        v, N, t = n.split("-")[0], n.split("-")[1], n.split("-t")[1]
        d = pan(r); mm = d.get("mesh", {})
        def me(c): return f"{mm[c][1]}, {fm(mm[c][2])}" if c in mm else "-"
        okc = f'{d["ok"]} / {d["err"]} / {d["panics"] if d["panics"] is not None else "?"}' if "ok" in d else "-"
        out.append(f'| {"cut (boolean)" if v == "b" else "profile hole"} | {N} | {t} | {r["status"]} | {fm(d["wall"]) if "wall" in d else "-"} | {okc} | {me("2")} | {me("0.5")} | {r["rss_mb"]} MB |')
    return "\n".join(out)
def build_info(k):
    p = f"{root}/logs/{k}/build.log"
    if not os.path.exists(p): return None
    t = open(p).read()
    el = re.findall(r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): ([\d:.]+)", t)
    def sec(x): parts = [float(y) for y in x.split(":")]; return sum(v * 60 ** i for i, v in enumerate(reversed(parts)))
    return round(sum(sec(x) for x in el)), re.findall(r"load (?:before|after) build: ([\d.]+)", t)
def lic(k):
    p = f"{root}/results/{k}.licenses.json"
    return json.load(open(p)) if os.path.exists(p) else None
md = []
md.append(open(f"{root}/harness/README-head.md").read())
md.append("\n## Side-by-side summary\n")
md.append("### Build, licence, unsafe\n\n| kernel | clean release build (s, wall, shared machine) | crates in tree (normal+build) | non-permissive licences in tree | unsafe in the kernel crate(s) | stable toolchain |\n|---|---|---|---|---|---|")
for k, name in K:
    if k == "bimcraft": md.append(f"| {name} | 340 s whole bimcraft-core test build (not comparable) | n/a | n/a (own code) | n/a | yes (1.100) |"); continue
    b = build_info(k); l = lic(k)
    if not l: continue
    us = "; ".join(f'{n}: {"forbid" if v["forbid_unsafe_lib_rs"] else "NOT forbidden"}, {v["unsafe_sites_in_src"]} unsafe sites' for n, v in l["roots"].items())
    np = "; ".join(l["non_permissive"]) or "none"
    md.append(f"| {name} | {b[0] if b else '?'} | {l['crate_count']} | {np} | {us} | yes (rustc 1.100, edition per crate) |")
md.append("\n### Forms, booleans, hostile input (outcome per case)\n")
cases = [n for (g, n) in R["arris"] if g == "forms" and n != "STEP"]
md.append("| case | " + " | ".join(n for k, n in K if k != "bimcraft") + " |\n|---|" + "---|" * (len(K) - 1))
for c in cases:
    md.append(f"| {c} | " + " | ".join(rel(R[k][("forms", c)]) if ("forms", c) in R[k] else "-" for k, _ in K if k != "bimcraft") + " |")
md.append("\nBIMCraft's mesh path has no B-rep forms to run singly; geo_bench builds the whole panel (see below). Its curved faces are fixed-tessellation meshes (no true curved surface, no chord control); booleans are mesh CSG.\n")
md.append("### 10,000 different panels (build wall seconds; `TIMEOUT` = over the 90 s budget; n=1000 in brackets when the 10,000 call timed out)\n")
md.append("Panel = L slab + revolved ring + ruled square-to-circle loft + one round through-void (see Method). Fornjot builds only the slab (no revolve, no loft); solidcore and Fornjot cannot mesh (solidcore) / only plane-circle (Fornjot) as stated.\n")
md.append("| kernel | variant | 1 thread | 8 threads | 72 threads | peak RSS at 72 thr | tris/panel @2 mm | @0.5 mm |\n|---|---|---|---|---|---|---|---|")
for k, name in K:
    if k == "bimcraft": continue
    for v, vn in (("b", "void by boolean cut"), ("c", "void as profile hole")):
        cells_ = []
        for t in (1, 8, 72):
            r = R[k].get(("panels", f"{v}-10000-t{t}")); r2 = R[k].get(("panels", f"{v}-1000-t{t}"))
            d = pan(r) if r else {}
            if r and "wall" in d and r["status"] != "ok": cells_.append(f'{fm(d["wall"])} s build (then {r["status"].split("(")[0]} while meshing)')
            elif r and r["status"] == "ok" and "wall" in d: cells_.append(f'{fm(d["wall"])} s')
            else:
                d2 = pan(r2) if r2 else {}
                st = r["status"].split("(")[0] if r else "-"
                cells_.append(f'{st}' + (f' [n=1000: {fm(d2["wall"])} s]' if "wall" in d2 and r2["status"] == "ok" else ""))
        r72 = R[k].get(("panels", f"{v}-10000-t72")); d72 = pan(r72) if r72 else {}
        mm = d72.get("mesh", {})
        trip = lambda c: (str(mm[c][1]) if c in mm else "-")
        md.append(f'| {name} | {vn} | {" | ".join(cells_)} | {r72["rss_mb"] if r72 else "-"} MB | {trip("2")} | {trip("0.5")} |')
for vi, vn in ((0, "no void"), (1, "void by mesh CSG")):
    c_ = []
    for t in (1, 8, 72):
        r = R["bimcraft"].get(("panels", f"geo_bench-10000-t{t}")); o = (r["stdout"] + r["stderr_tail"]) if r else ""
        m_ = re.findall(r"all cores ([\d.]+) s", o); c_.append(f"{m_[vi]} s" if len(m_) > vi else "?")
    r = R["bimcraft"].get(("panels", "geo_bench-10000-t72")); o = r["stdout"] + r["stderr_tail"] if r else ""
    tr = re.findall(r"(\d+) triangles a panel", o)
    md.append(f'| BIMCraft mesh path (own panel, geo_bench) | {vn} | {" | ".join(c_)} | {r["rss_mb"] if r else "?"} MB | {tr[vi] if len(tr) > vi else "?"} (fixed, no chord control) | same |')
md.append("\nBIMCraft mesh path (geo_bench, its own panel; all-cores build of 10,000 panels): see its table. For scale: 10,000 panels, 1 thread: no void ~1 s, with void ~3 s, ~344 / 476 tris per panel.\n")
for k, name in K:
    md.append(f"\n## {name}\n")
    l = lic(k); b = build_info(k)
    if l:
        md.append(f"- crates in tree: {l['crate_count']}; non-permissive: {', '.join(l['non_permissive']) or 'none'}; licence histogram {l['licence_histogram']}")
        for n, v in l["roots"].items(): md.append(f"- `{n}` {v['version']}: {v['license']}, forbid(unsafe_code) in lib.rs: {v['forbid_unsafe_lib_rs']}, unsafe sites in src: {v['unsafe_sites_in_src']}, rust-version {v['rust_version']}, edition {v['edition']}")
    if b: md.append(f"- clean release build: {b[0]} s wall; machine load average at build start/end: {b[1]}")
    for g, title in (("forms", "Forms, booleans, hostile input"), ("nurbs", "NURBS table"), ("panels", "10,000 panels")):
        t = forms_table(k, g) if g != "panels" else panels_table(k)
        if t: md.append(f"\n### {title}\n\n{t}")
hard = f"{root}/results/ade-hardcases/README.md"   # the hard-cases section, written by harness/hardcases.py readme
if os.path.exists(hard): md.append("\n" + open(hard).read())
open(f"{root}/README.md", "w").write("\n".join(md) + "\n" + open(f"{root}/harness/README-tail.md").read())
