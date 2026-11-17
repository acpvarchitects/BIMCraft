#!/bin/bash
# usage: run-kernel.sh <name> <suite.json> <root-crates-csv> <dir>...   (dirs = cargo projects built into one target dir)
set -u
export PATH=/mnt/data/emiliano_capasso/tools-venv/bin:$HOME/.cargo/bin:$PATH; export CMAKE_POLICY_VERSION_MINIMUM=3.5
name=$1; suite=$2; roots=$3; shift 3
R=/mnt/data/emiliano_capasso/bench-kernels
export CARGO_TARGET_DIR=/mnt/data/emiliano_capasso/cargo-targets/bench-$name
free=$(df -BG --output=avail /mnt/data | tail -1 | tr -dc 0-9)
echo "free on /mnt/data before $name: ${free} GB"; [ "$free" -lt 60 ] && { echo "STOP: under 60 GB free"; exit 3; }
mkdir -p $R/logs/$name $R/results; rm -f $R/results/$name.jsonl
{ echo "load before build: $(cat /proc/loadavg)"; } > $R/logs/$name/build.log
first=1
for d in "$@"; do
  ( cd $R/$d && /usr/bin/time -v cargo build --release 2>&1 ) >> $R/logs/$name/build.log 2>&1
done
echo "load after build: $(cat /proc/loadavg)" >> $R/logs/$name/build.log
grep -E "Elapsed \(wall|Maximum resident" $R/logs/$name/build.log
( cd $R/$1 && python3 $R/harness/licenses.py . ${roots//,/ } ) > $R/results/$name.licenses.json 2> $R/logs/$name/licenses.err
du -sh $CARGO_TARGET_DIR | tee -a $R/logs/$name/build.log
python3 $R/harness/run.py $name $CARGO_TARGET_DIR/release $suite
du -sh $CARGO_TARGET_DIR > $R/logs/$name/target-size.txt
