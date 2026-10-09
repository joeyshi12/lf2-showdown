// Export every function: address, size, callers, callees, and decompiled C.
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import java.io.*;
import java.util.*;

public class ExportFunctions extends GhidraScript {
    @Override
    public void run() throws Exception {
        String outDir = getScriptArgs()[0];
        new File(outDir + "/decomp").mkdirs();
        DecompInterface ifc = new DecompInterface();
        ifc.openProgram(currentProgram);
        PrintWriter tsv = new PrintWriter(new FileWriter(outDir + "/functions.tsv"));
        tsv.println("address\tname\tsize\tcallers\tcallees");
        FunctionIterator it = currentProgram.getFunctionManager().getFunctions(true);
        while (it.hasNext() && !monitor.isCancelled()) {
            Function f = it.next();
            StringJoiner callers = new StringJoiner(",");
            for (Function c : f.getCallingFunctions(monitor)) callers.add(c.getEntryPoint().toString());
            StringJoiner callees = new StringJoiner(",");
            for (Function c : f.getCalledFunctions(monitor)) callees.add(c.getEntryPoint().toString());
            tsv.printf("%s\t%s\t%d\t%s\t%s%n", f.getEntryPoint(), f.getName(),
                f.getBody().getNumAddresses(), callers, callees);
            DecompileResults r = ifc.decompileFunction(f, 120, monitor);
            if (r.decompileCompleted()) {
                try (PrintWriter w = new PrintWriter(new FileWriter(
                        outDir + "/decomp/" + f.getEntryPoint() + ".c"))) {
                    w.print(r.getDecompiledFunction().getC());
                }
            }
        }
        tsv.close();
    }
}
