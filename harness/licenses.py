#!/usr/bin/env python3
"""usage: licenses.py <crate-dir> <root-crate-name>... ; prints JSON: crate count, licences, non-permissive, forbid(unsafe) for roots.
Reads cargo metadata only; never runs anything from the dependency sources (greps lib.rs text only)."""
import json, subprocess, sys, os, re
d = sys.argv[1]; roots = sys.argv[2:]
md = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--manifest-path", f"{d}/Cargo.toml", "--locked" if os.path.exists(f"{d}/Cargo.lock") else "--offline"], text=True))
pk = {p["id"]: p for p in md["packages"]}
nodes = {n["id"]: n for n in md["resolve"]["nodes"]}
me = md["resolve"]["root"]
# normal+build deps only (no dev)
seen, st = set(), [me]
while st:
    i = st.pop()
    if i in seen: continue
    seen.add(i)
    for dep in nodes[i]["deps"]:
        if any(k["kind"] in (None, "build") for k in dep["dep_kinds"]): st.append(dep["pkg"])
deps = [pk[i] for i in seen if i != me]
PERM = re.compile(r"^(MIT|Apache-2\.0|BSD-[23]-Clause|ISC|Zlib|Unlicense|CC0-1\.0|0BSD|Unicode-3\.0|Unicode-DFS-2016|BSL-1\.0|MIT-0|WITH LLVM-exception|NCSA|OpenSSL|Apache-2\.0 WITH LLVM-exception)$")
def permissive(lic):
    if not lic: return False
    # an OR expression is permissive if any alternative is wholly permissive
    for alt in re.split(r"\s+OR\s+|/", lic.replace("(", "").replace(")", "")):
        parts = [x.strip() for x in re.split(r"\s+AND\s+", alt)]
        if all(PERM.match(x) or x.startswith("Apache-2.0 WITH") for x in parts): return True
    return False
flag = sorted(f'{p["name"]} {p["version"]}: {p["license"] or "NO LICENSE FIELD ("+str(p.get("license_file"))+")"}' for p in deps if not permissive(p["license"]))
lic = {}
for p in deps: lic[p["license"] or "NONE"] = lic.get(p["license"] or "NONE", 0) + 1
res = dict(crate_count=len(deps), non_permissive=flag, licence_histogram=dict(sorted(lic.items(), key=lambda x: -x[1])[:12]), roots={})
for p in deps:
    if p["name"] in roots:
        src = os.path.dirname(p["manifest_path"]); forbid = None; unsafe_n = 0
        for dp, _, fs in os.walk(src):
            for f in fs:
                if f.endswith(".rs"):
                    t = open(os.path.join(dp, f), errors="replace").read()
                    unsafe_n += len(re.findall(r"\bunsafe\b\s*(\{|fn|impl|trait)", t))
        lib = os.path.join(src, "src", "lib.rs")
        if os.path.exists(lib):
            t = open(lib, errors="replace").read(); forbid = bool(re.search(r"#!\[(forbid|deny)\(unsafe_code\)\]", t))
        res["roots"][p["name"]] = dict(version=p["version"], license=p["license"], forbid_unsafe_lib_rs=forbid, unsafe_sites_in_src=unsafe_n, rust_version=p.get("rust_version"), edition=p["edition"])
print(json.dumps(res, indent=1))
