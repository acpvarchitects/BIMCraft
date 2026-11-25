#!/usr/bin/env python3
"""The hard-cases suite: one list of cases, the expected answer for each, and the classifier.

Every kernel's `hardcases` binary takes a case name as argv[1] and prints one JSON line:
  outcome   "solid" | "error" | "empty" | "no_op" | "panic"
  err       the kernel's message for error / no_op
  ms        wall time of the operation(s), milliseconds
  volume    the result's volume (vsrc: "exact" = the kernel's own integrator, "mesh" = from its tessellation)
  ref       for cases checked against the kernel's own input (a loft, a panel): that input's volume
  faces, edges, valid (the kernel's own validity check, null where it has none), mesh, step
This file turns that line, plus run.py's process status, into one of
  ok | refused | wrong | crash | timeout | no op
usage: hardcases.py suites <kernel> <bin>      -> writes harness/suites/<kernel>-hard.json
       hardcases.py table <results.jsonl>...   -> markdown table on stdout
       hardcases.py readme                     -> results/ade-hardcases/{table.md,README.md} from the three kernels' records
"""
import json, math, sys

PI = math.pi
S = math.tan(math.radians(0.1))
def cup(t): return 1000 * 600 * 750 - (1000 - 2 * t) * (600 - 2 * t) * (750 - t)
def shoelace(p): return abs(sum(p[i][0] * p[(i + 1) % len(p)][1] - p[(i + 1) % len(p)][0] * p[i][1] for i in range(len(p)))) / 2
def prismatoid(a, b, h):  # ruled loft between corresponding polygon vertices: the section area is quadratic, Simpson is exact
    m = [((x0 + x1) / 2, (y0 + y1) / 2) for (x0, y0), (x1, y1) in zip(a, b)]
    return h / 6 * (shoelace(a) + 4 * shoelace(m) + shoelace(b))
RECT = [(-200, -100), (200, -100), (200, 100), (-200, 100)]
TOP_1B = [(-100, 0), (100, -50), (100, 50), (-100, 0)]
CONE = PI * 300**2 * 500 / 3
def spandrel_ring(rc, r):  # the concave round at the foot of a boss of radius rc, by Pappus
    a = (1 - PI / 4) * r * r; xbar = r * (10 - 3 * PI) / (12 - 3 * PI)
    return 2 * PI * (rc + xbar) * a

# expectation kinds: V (closed form, rtol from the geometry), refuse, refuse_or V, empty_or_refuse,
# ref_minus (own input's volume minus a closed form), between (bounds from own input's volume)
CASES = [
 # 1 degenerate edges and points
 ("1a loft rect to apex", dict(V=400 * 200 * 300 / 3, planar=True)),
 ("1b loft side shrinks to zero", dict(V=prismatoid(RECT, TOP_1B, 300), planar=False)),
 ("1c revolve profile touching axis", dict(V=PI * 300**2 * 500)),
 ("1d revolve profile crossing axis", dict(refuse=True)),
 ("1e revolve cone apex on axis", dict(V=CONE)),
 ("1f cone cut through its apex", dict(V=CONE / 2)),
 ("1g cone apex inside a box fuse", dict(V=CONE + 100**3 - PI * 30**2 * 50 / 3)),
 # 2 tangency and coincidence
 ("2a cylinder tangent to a face fuse", dict(V=1000 * 600 * 200 + PI * 100**2 * 1000)),
 ("2b cylinder tangent inside a face cut", dict(V=1000 * 600 * 750 - PI * 100**2 * 750)),
 ("2c equal cylinders crossing at 90deg fuse", dict(V=2 * PI * 100**2 * 1000 - 16 * 100**3 / 3)),
 ("2d coplanar faces partly overlapping fuse", dict(V=2 * 1000 * 600 * 750 - 500 * 300 * 750, planar=True)),
 ("2e boxes stacked sharing a face fuse", dict(V=1000 * 600 * 1000, planar=True)),
 ("2f void face on a body face cut", dict(V=1000 * 600 * 750 - 600 * 400 * 400, planar=True)),
 # 3 thin and near-degenerate
 ("3a cup wall 1mm", dict(V=cup(1.0), planar=True)),
 ("3b cup wall 0.1mm", dict(V=cup(0.1), planar=True)),
 ("3c cup wall 0.01mm", dict(V=cup(0.01), planar=True)),
 ("3d sliver 0.01mm from a near-miss cut", dict(V=0.01 * 600 * 750, planar=True)),
 ("3e plane cut at 0.1deg", dict(V=750 * (300 * 1000 + S * 1000**2 / 2), planar=True)),
 ("3f wedge of 0.1deg", dict(V=750 * S * 1000**2 / 2, planar=True)),
 ("3g box minus a thin NURBS panel", dict(ref_minus=None, base=1000 * 600 * 750)),
 # 4 curved booleans that need a general intersection
 ("4a cylinder minus a cylinder at 45deg", dict(V=PI * 100**2 * 2000 - 16 * 100**3 / (3 * math.sin(math.radians(45))))),
 ("4b ruled loft minus a cylinder", dict(ref_minus=PI * 100**2 * 900)),
 ("4c two lofts fused", dict(between=(1.0, 2.0))),
 ("4d revolved ring cut by an inclined plane", dict(V=PI * (300**2 - 100**2) * 500 / 2)),
 ("4e cylinder cut by an inclined plane", dict(V=PI * 300**2 * 250)),
 # 5 fillets and offsets
 ("5a fillet plane-plane", dict(V=1000 * 600 * 750 - (1 - PI / 4) * 50**2 * 750)),
 ("5b fillet plane-cylinder", dict(V=1000 * 1000 * 200 + PI * 100**2 * 400 + spandrel_ring(100, 20))),
 ("5c fillet NURBS-plane", dict(between=(1 - 1e-2, 1.0), strict_less=True)),
 ("5d fillet NURBS-NURBS", dict(between=(1 - 1e-2, 1.0), strict_less=True)),
 ("5e shell a cylinder open at the top", dict(V=PI * 300**2 * 500 - PI * 290**2 * 490)),
 ("5f offset a cylindrical face", dict(V=PI * 310**2 * 500)),
 # 6 hostile input: refusing is fine, a wrong or empty "valid" solid is not
 ("6a box with a NaN corner", dict(refuse=True)),
 ("6b box of zero thickness", dict(refuse=True)),
 ("6c box with inverted corners", dict(refuse_or=1000 * 600 * 750, planar=True)),
 ("6d cylinder of radius 0", dict(refuse=True)),
 ("6e cylinder of radius NaN", dict(refuse=True)),
 ("6f cylinder of negative radius", dict(refuse_or=PI * 100**2 * 750)),
 ("6g unit box at 1e9 from the origin", dict(V=1.0, rtol=1e-3)),
 ("6h box fused with itself", dict(V=1000 * 600 * 750, planar=True)),
 ("6i box cut by itself", dict(empty_or_refuse=True)),
 ("6j extrude a NaN profile", dict(refuse=True)),
 ("6k extrude by zero", dict(refuse=True)),
 ("6l extrude a self-intersecting profile", dict(refuse_or=2 * 25 * 5, planar=True)),
]
SPEC = dict(CASES)

def rtol(e, vsrc):
    if "rtol" in e: return e["rtol"]
    if e.get("planar"): return 1e-6          # planar faces: any exact or mesh volume is exact to rounding
    return 1e-5 if vsrc == "exact" else 2e-3  # curved: an integrator to 1e-5; a 0.5 mm-chord mesh to 0.2 %

def classify(rec):
    """rec: run.py record. Returns (status, note)."""
    st = rec["status"]
    if st.startswith("TIMEOUT"): return "timeout", st
    if st != "ok": return "crash", (st + " " + rec.get("stderr_tail", "")[-120:]).strip()
    try: r = json.loads(rec["stdout"].strip().splitlines()[-1])
    except Exception: return "crash", "no result line"
    e = SPEC[rec["name"]]; o = r["outcome"]
    if o == "panic": return "crash", "panic: " + (r.get("err") or "")[:120]
    if o == "no_op": return "no op", (r.get("err") or "")[:120]
    if o == "error": return "refused", (r.get("err") or "")[:120]
    if o == "empty":
        return ("ok", "empty, as expected") if e.get("empty_or_refuse") else ("wrong", "empty result")
    # a solid came back
    v, vs = r.get("volume"), r.get("vsrc")
    inval = r.get("valid") is False
    if e.get("refuse"): return "wrong", f"a solid where a refusal was due (volume {v})"
    if e.get("empty_or_refuse"): return "wrong", f"a solid of volume {v} where the result is empty"
    if v is None or not math.isfinite(v) or v <= 0: return "wrong", f"volume {v}"
    exp = None
    if "V" in e: exp = e["V"]
    elif "refuse_or" in e: exp = e["refuse_or"]
    elif "ref_minus" in e:
        ref = r.get("ref")
        if ref is None: return "wrong", "no input volume"
        exp = (e["base"] - ref) if e.get("base") else (ref - e["ref_minus"])
    if exp is not None:
        tol = rtol(e, vs); err = abs(v - exp) / abs(exp)
        if err > tol: return "wrong", f"volume {v:.6g}, expected {exp:.6g} (off {err:.1e}, tol {tol:.0e})"
        note = f"volume off {err:.1e}"
    else:
        ref = r.get("ref"); lo, hi = e["between"]
        if ref is None: return "wrong", "no input volume"
        if not (lo * ref < v <= hi * ref) or (e.get("strict_less") and v >= ref):
            return "wrong", f"volume {v:.6g} outside ({lo}, {hi}] x input {ref:.6g}"
        note = f"volume {v / ref:.6f} x input"
    if inval: return "wrong", note + "; the kernel's own check says invalid"
    if r.get("mesh", "").startswith("err"): note += "; mesh failed"
    return "ok", note

def table(paths):
    recs = [json.loads(l) for p in paths for l in open(p)]
    ks = list(dict.fromkeys(r["kernel"] for r in recs)); by = {(r["kernel"], r["name"]): r for r in recs}
    out = ["| case | " + " | ".join(ks) + " |", "|---|" + "---|" * len(ks)]
    tally = {k: {} for k in ks}; notes = []
    for name, _ in CASES:
        row = []
        for k in ks:
            r = by.get((k, name))
            if not r: row.append("—"); continue
            s, n = classify(r)
            tally[k][s] = tally[k].get(s, 0) + 1
            try: ms = json.loads(r["stdout"].splitlines()[-1]).get("ms")
            except Exception: ms = None
            t = f" {ms:.1f} ms" if isinstance(ms, (int, float)) and s in ("ok", "wrong", "refused") else (f" {r['wall_s']:.0f} s" if s == "timeout" else "")
            row.append(f"{'**' + s + '**' if s in ('wrong', 'crash', 'timeout') else s}{t}")
            if s != "ok" and s != "no op": notes.append(f"- {name}, {k}: {s}: {n}")
        out.append(f"| {name} | " + " | ".join(row) + " |")
    order = ["ok", "refused", "no op", "wrong", "crash", "timeout"]
    out.append("| **total** | " + " | ".join(", ".join(f"{tally[k][s]} {s}" for s in order if tally[k].get(s)) for k in ks) + " |")
    return "\n".join(out) + "\n\nNotes (every result other than ok and no op):\n\n" + "\n".join(notes) + "\n"

if __name__ == "__main__":
    if sys.argv[1] == "suites":
        k, b = sys.argv[2], sys.argv[3]
        json.dump([{"group": "hard", "name": n, "bin": b, "args": [n]} for n, _ in CASES], open(f"{sys.path[0]}/suites/{k}-hard.json", "w"), indent=0)
    elif sys.argv[1] == "table":
        print(table(sys.argv[2:]), end="")
    elif sys.argv[1] == "readme":
        import os
        root = os.path.dirname(sys.path[0]); d = f"{root}/results/ade-hardcases"
        tb = table([f"{d}/{k}.jsonl" for k in ("arris", "sc-main", "cadcore") if os.path.exists(f"{d}/{k}.jsonl")])
        tb = tb.replace("| case | arris | sc-main | cadcore |", "| case | arris 0.12.0 | solidcore main | CadCore main |")
        open(f"{d}/table.md", "w").write(tb)
        head = open(f"{sys.path[0]}/README-hardcases-head.md").read()
        tail = open(f"{sys.path[0]}/README-hardcases-tail.md").read() if os.path.exists(f"{sys.path[0]}/README-hardcases-tail.md") else ""
        open(f"{d}/README.md", "w").write(head + "### Results\n\n" + tb + "\n" + tail)
