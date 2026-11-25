#!/usr/bin/env python3
"""run.py for Windows (the workstation runs): every bench call in a child process with a time budget.
usage: run_win.py <kernel> <bin-dir> <suite.json> [only-group]
Same suite format, same records and the same files as run.py (logs/<kernel>/<group>__<name>.{out,err,time},
results/<kernel>.jsonl, appended; BENCH_LOGS, BENCH_RESULTS and BENCH_BUDGET as there). What differs, since
Windows has neither /usr/bin/time nor RLIMIT_AS: rss_mb is the child's peak working set and the .time file
holds its CPU time (user + kernel) from GetProcessTimes; there is no address-space cap; a timeout kills the
child (TerminateProcess) and keeps what it printed before."""
import ctypes, json, os, re, subprocess, sys, time
from ctypes import wintypes

kernel, bindir, suite = sys.argv[1], sys.argv[2], sys.argv[3]
only = sys.argv[4] if len(sys.argv) > 4 else None
root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
resd = os.environ.get("BENCH_RESULTS", f"{root}/results"); logd = os.path.join(os.environ.get("BENCH_LOGS", f"{root}/logs"), kernel)
os.makedirs(logd, exist_ok=True); os.makedirs(resd, exist_ok=True)
BUDGET = int(os.environ.get("BENCH_BUDGET", "90"))

class PMC(ctypes.Structure):
    _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [(f, ctypes.c_size_t) for f in (
        "PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage", "QuotaPagedPoolUsage",
        "QuotaPeakNonPagedPoolUsage", "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage")]

def usage(p):
    """(peak working set in bytes, cpu seconds) of a child that has ended but whose handle is still open."""
    h = wintypes.HANDLE(int(p._handle))
    c = PMC(); c.cb = ctypes.sizeof(c)
    peak = c.PeakWorkingSetSize if ctypes.windll.psapi.GetProcessMemoryInfo(h, ctypes.byref(c), c.cb) else None
    ft = [wintypes.FILETIME() for _ in range(4)]
    cpu = None
    if ctypes.windll.kernel32.GetProcessTimes(h, *[ctypes.byref(f) for f in ft]):
        cpu = sum((f.dwHighDateTime << 32 | f.dwLowDateTime) for f in ft[2:]) / 1e7
    return peak, cpu

for c in json.load(open(suite)):
    if only and c["group"] != only: continue
    tag = re.sub(r"[^A-Za-z0-9._-]+", "_", f'{c["group"]}__{c["name"]}')
    env = dict(os.environ); env.pop("RUST_BACKTRACE", None)
    if c.get("threads"): env["RAYON_NUM_THREADS"] = str(c["threads"])
    env.update(c.get("env", {}))
    cmd = [os.path.join(bindir, c["bin"] + ".exe")] + c.get("args", [])
    t = time.time(); status = "ok"
    with open(f"{logd}/{tag}.out", "w") as so, open(f"{logd}/{tag}.err", "w") as se:
        p = subprocess.Popen(cmd, stdout=so, stderr=se, env=env)
        try: rc = p.wait(timeout=BUDGET)
        except subprocess.TimeoutExpired:
            p.kill(); p.wait(); rc = None; status = "TIMEOUT>%ds" % BUDGET
    wall = time.time() - t
    peak, cpu = usage(p)
    open(f"{logd}/{tag}.time", "w").write(f"wall_s {wall:.3f}\ncpu_s {cpu}\npeak_working_set_bytes {peak}\nexit {rc}\n")
    err = open(f"{logd}/{tag}.err", errors="replace").read()
    if rc is not None and rc != 0:
        if "memory allocation" in err or "out of memory" in err.lower() or "alloc" in err and "failed" in err: status = "OOM"
        elif rc == 101 or "panicked" in err: status = f"CRASH(exit {rc}, panic)"
        else: status = f"CRASH(exit {rc:#x})" if rc > 255 or rc < 0 else f"exit {rc}"
    out = open(f"{logd}/{tag}.out", errors="replace").read()
    rec = dict(kernel=kernel, group=c["group"], name=c["name"], threads=c.get("threads"), status=status, wall_s=round(wall, 3),
               rss_mb=round(peak / 2**20) if peak else None, cpu_s=round(cpu, 2) if cpu is not None else None,
               stdout=out.strip()[:4000], stderr_tail=err.strip()[-300:])
    open(f"{resd}/{kernel}.jsonl", "a").write(json.dumps(rec) + "\n")
    print(f'[{kernel}] {tag}: {status} {wall:.2f}s cpu={rec["cpu_s"]}s rss={rec["rss_mb"]}MB', flush=True)
