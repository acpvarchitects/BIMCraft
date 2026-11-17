#!/usr/bin/env python3
"""Run every bench call in a child process with a time budget and an address-space cap.
usage: run.py <kernel> <bin-dir> <suite.json> [only-group]
suite.json: list of {"group","name","bin","args":[...],"threads":N|null}
Writes logs/<kernel>/<group>__<name>.{out,err,time} and results/<kernel>.jsonl (append)."""
import json, os, re, resource, subprocess, sys, time
kernel, bindir, suite = sys.argv[1], sys.argv[2], sys.argv[3]
only = sys.argv[4] if len(sys.argv) > 4 else None
root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
logd = f"{root}/logs/{kernel}"; os.makedirs(logd, exist_ok=True); os.makedirs(f"{root}/results", exist_ok=True)
BUDGET = int(os.environ.get("BENCH_BUDGET", "90")); AS = int(os.environ.get("BENCH_AS_GB", "96")) << 30
def cap(): resource.setrlimit(resource.RLIMIT_AS, (AS, AS))
for c in json.load(open(suite)):
    if only and c["group"] != only: continue
    tag = re.sub(r"[^A-Za-z0-9._-]+", "_", f'{c["group"]}__{c["name"]}')
    env = dict(os.environ); env.pop("RUST_BACKTRACE", None)
    if c.get("threads"): env["RAYON_NUM_THREADS"] = str(c["threads"])
    env.update(c.get("env", {}))
    cmd = ["/usr/bin/time", "-v", "-o", f"{logd}/{tag}.time", f"{bindir}/{c['bin']}"] + c.get("args", [])
    t = time.time(); status = "ok"
    with open(f"{logd}/{tag}.out", "w") as so, open(f"{logd}/{tag}.err", "w") as se:
        p = subprocess.Popen(cmd, stdout=so, stderr=se, env=env, preexec_fn=cap, start_new_session=True)
        try: rc = p.wait(timeout=BUDGET)
        except subprocess.TimeoutExpired:
            subprocess.run(["pkill", "-9", "-P", str(p.pid)])   # kill the bench child only, so /usr/bin/time still reports its peak RSS
            try: p.wait(timeout=15)
            except subprocess.TimeoutExpired: os.killpg(p.pid, 9); p.wait()
            rc = None; status = "TIMEOUT>%ds" % BUDGET
    wall = time.time() - t
    err = open(f"{logd}/{tag}.err", errors="replace").read()
    tm = open(f"{logd}/{tag}.time", errors="replace").read() if os.path.exists(f"{logd}/{tag}.time") else ""
    rss = re.search(r"Maximum resident set size \(kbytes\): (\d+)", tm)
    rss = int(rss.group(1)) if rss else None
    if rc is not None and rc != 0:
        if "memory allocation" in err or "out of memory" in err.lower() or "alloc" in err and "failed" in err: status = "OOM"
        elif rc in (101, 134) or "panicked" in err: status = f"CRASH(exit {rc}, panic)"
        elif rc < 0 or rc > 128: status = f"CRASH(exit {rc})"
        else: status = f"exit {rc}"
    out = open(f"{logd}/{tag}.out", errors="replace").read()
    rec = dict(kernel=kernel, group=c["group"], name=c["name"], threads=c.get("threads"), status=status, wall_s=round(wall, 3),
               rss_mb=round(rss / 1024) if rss else None, stdout=out.strip()[:4000], stderr_tail=err.strip()[-300:])
    open(f"{root}/results/{kernel}.jsonl", "a").write(json.dumps(rec) + "\n")
    print(f'[{kernel}] {tag}: {status} {wall:.2f}s rss={rec["rss_mb"]}MB', flush=True)
