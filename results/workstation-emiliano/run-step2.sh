#!/bin/bash
# Step 2 on Emiliano's laptop: the 43 hard cases (CadCore: its 15) at 90 s, then the panel for arris 0.12 and
# solidcore main at 900 s a call. Everything built with --features mimalloc (MSVC).
set -u
R=$(cd "$(dirname "$0")/../.." && pwd)   # the bench checkout
W=$R/results/workstation-emiliano
cd $R
state() {
  echo "== $1 $(date -Iseconds)"
  powershell -NoProfile -Command "\$c=(Get-CimInstance Win32_Processor | Measure-Object LoadPercentage -Average).Average; \$os=Get-CimInstance Win32_OperatingSystem; \$b=Get-CimInstance Win32_Battery; 'cpu load ' + \$c + ' %, free RAM ' + [math]::Round(\$os.FreePhysicalMemory/1MB,1) + ' GB, battery ' + \$b.EstimatedChargeRemaining + ' % status ' + \$b.BatteryStatus; Get-Process | Group-Object ProcessName | ForEach-Object { [pscustomobject]@{n=\$_.Name; c=\$_.Count; gb=[math]::Round((\$_.Group | Measure-Object WorkingSet64 -Sum).Sum/1GB,2)} } | Sort-Object gb -Descending | Select-Object -First 8 | ForEach-Object { '  ' + \$_.n + ' x' + \$_.c + ' ' + \$_.gb + ' GB' }"
}
{
state "before"
export BENCH_BUDGET=90 BENCH_RESULTS=$W/hardcases BENCH_LOGS=$W/logs/hardcases
rm -f $W/hardcases/*.jsonl
python harness/run_win.py arris arris12-t/target/release harness/suites/arris-hard.json
python harness/run_win.py sc-main sc-main-t/target/release harness/suites/sc-main-hard.json
python harness/run_win.py cadcore cadcore-t/target/release harness/suites/cadcore-hard-ops.json
state "after hard cases"
export BENCH_BUDGET=900 BENCH_RESULTS=$W/panels BENCH_LOGS=$W/logs/panels
rm -f $W/panels/*.jsonl
python harness/run_win.py arris arris12-t/target/release harness/suites/panels-workstation.json
state "after arris panels"
python harness/run_win.py sc-main sc-main-t/target/release harness/suites/panels-workstation.json
state "after"
echo done
} 2>&1
