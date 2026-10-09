#!/bin/sh
# Full re-import and export. Usage: tools/ghidra/run.sh /path/to/lf2.exe
# Writes re/out/{functions.tsv,xrefs.tsv,decomp/*.c}. Takes about 5 minutes.
set -e
cd "$(dirname "$0")/../.."
EXE=${1:-$LF2_DIR/lf2.exe}
rm -rf re/ghidra-project re/out.new
mkdir -p re/ghidra-project re/out.new
/opt/ghidra/support/analyzeHeadless re/ghidra-project lf2 -import "$EXE" \
  -scriptPath tools/ghidra -preScript EnableParamId.java \
  -postScript ApplyNames.java "$PWD/re/names.tsv" \
  -postScript ApplyTypes.java "$PWD/re/types.txt" "$PWD/re/signatures.txt" \
  -postScript ApplyFixups.java \
  -postScript ExportFunctions.java "$PWD/re/out.new" \
  -postScript ExportXrefs.java "$PWD/re/out.new" > re/ghidra.log 2>&1
rm -rf re/out && mv re/out.new re/out
echo done
