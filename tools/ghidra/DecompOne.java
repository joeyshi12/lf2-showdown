// Print the decompile of the functions given as arguments (hex addresses).
// Optional first argument "nofix" clears the call-fixups first, "absonly" keeps only abs.
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.Function;

public class DecompOne extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] a = getScriptArgs();
        int start = 0;
        if (a[0].equals("nofix") || a[0].equals("absonly")) {
            getFunctionAt(toAddr(0x4450d0)).setCallFixup(null);
            if (a[0].equals("nofix")) getFunctionAt(toAddr(0x4034e0)).setCallFixup(null);
            start = 1;
        }
        DecompInterface ifc = new DecompInterface();
        ifc.openProgram(currentProgram);
        for (int i = start; i < a.length; i++) {
            Function f = getFunctionAt(toAddr(Long.parseLong(a[i], 16)));
            DecompileResults r = ifc.decompileFunction(f, 120, monitor);
            println("=====" + a[i] + "\n" + r.getDecompiledFunction().getC());
        }
    }
}
