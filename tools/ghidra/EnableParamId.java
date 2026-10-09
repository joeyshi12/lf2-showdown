// Pre-script: turn on the Decompiler Parameter ID analyzer so that function
// prototypes (including __thiscall arguments) are committed before export.
import ghidra.app.script.GhidraScript;

public class EnableParamId extends GhidraScript {
    @Override
    public void run() throws Exception {
        setAnalysisOption(currentProgram, "Decompiler Parameter ID", "true");
    }
}
