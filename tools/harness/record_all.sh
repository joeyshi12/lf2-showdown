#!/bin/sh
# Record and pack every scenario given on the command line (names without .txt).
# Runs one at a time; meant to be started detached. Log: scratch/record_all.log
cd "$(dirname "$0")/../.."
. ./scratch.env
for s in "$@"; do
  echo "== $s $(date +%T)"
  python3 tools/harness/record.py traces/scenarios/$s.txt scratch/$s.lf2t && \
    python3 tools/harness/pack_trace.py scratch/$s.lf2t traces/$s.lf2t
done
echo "== done $(date +%T)"
