// For every function: imported APIs it calls, strings it references, and data
// addresses (globals) it reads or writes. One TSV row per function.
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.address.*;
import ghidra.program.model.mem.*;
import java.io.*;
import java.util.*;

public class ExportXrefs extends GhidraScript {
    @Override
    public void run() throws Exception {
        String outDir = getScriptArgs()[0];
        PrintWriter tsv = new PrintWriter(new FileWriter(outDir + "/xrefs.tsv"));
        tsv.println("address\timports\tstrings\tglobals");
        Listing listing = currentProgram.getListing();
        ReferenceManager rm = currentProgram.getReferenceManager();
        SymbolTable st = currentProgram.getSymbolTable();
        MemoryBlock text = currentProgram.getMemory().getBlock(".text");
        for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
            TreeSet<String> imps = new TreeSet<>(), strs = new TreeSet<>(), globs = new TreeSet<>();
            for (Instruction ins : listing.getInstructions(f.getBody(), true)) {
                for (Reference r : ins.getReferencesFrom()) {
                    Address to = r.getToAddress();
                    Symbol s = st.getPrimarySymbol(to);
                    if (to.isExternalAddress() || (s != null && s.isExternal())) {
                        imps.add(s != null ? s.getName() : to.toString()); continue;
                    }
                    Data d = listing.getDataAt(to);
                    if (d != null && d.isPointer()) {
                        Object v = d.getValue();
                        if (v instanceof Address) {
                            Symbol ps = st.getPrimarySymbol((Address) v);
                            if (ps != null && ps.isExternal()) { imps.add(ps.getName()); continue; }
                        }
                    }
                    if (d != null && d.hasStringValue()) {
                        String sv = String.valueOf(d.getValue()).replace("\t"," ").replace("\n","\\n").replace("\r","\\r");
                        if (sv.length() > 60) sv = sv.substring(0, 60);
                        strs.add(sv); continue;
                    }
                    if (r.getReferenceType().isData() && to.isMemoryAddress() && !text.contains(to))
                        globs.add(to.toString() + (r.getReferenceType().isWrite() ? "w" : "r"));
                }
            }
            tsv.printf("%s\t%s\t%s\t%s%n", f.getEntryPoint(), String.join(",", imps),
                String.join(" | ", strs), String.join(",", globs));
        }
        tsv.close();
    }
}
