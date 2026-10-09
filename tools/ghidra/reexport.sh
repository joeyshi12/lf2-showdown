#!/bin/sh
# Apply re/names.tsv, re/types.txt and re/signatures.txt to the existing Ghidra project
# and export again (no re-import). Takes about 3 minutes.
set -e
cd "$(dirname "$0")/../.."
rm -rf re/out.new && mkdir -p re/out.new
/opt/ghidra/support/analyzeHeadless re/ghidra-project lf2 -process lf2.exe -noanalysis \
  -scriptPath tools/ghidra \
  -postScript ApplyNames.java "$PWD/re/names.tsv" \
  -postScript ApplyTypes.java "$PWD/re/types.txt" "$PWD/re/signatures.txt" \
  -postScript ApplyFixups.java \
  -postScript ExportFunctions.java "$PWD/re/out.new" \
  -postScript ExportXrefs.java "$PWD/re/out.new" > re/ghidra.log 2>&1
rm -rf re/out && mv re/out.new re/out
echo done
