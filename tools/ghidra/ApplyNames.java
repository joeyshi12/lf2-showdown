// Rename functions from re/names.tsv (address<TAB>name<TAB>...). Lines starting
// with '#' and the header are skipped. Missing file is not an error.
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.SourceType;
import java.io.*;
import java.nio.file.*;

public class ApplyNames extends GhidraScript {
    @Override
    public void run() throws Exception {
        Path p = Paths.get(getScriptArgs()[0]);
        if (!Files.exists(p)) return;
        for (String line : Files.readAllLines(p)) {
            if (line.startsWith("#") || line.startsWith("address")) continue;
            String[] f = line.split("\t");
            if (f.length < 2 || f[1].isEmpty() || f[1].startsWith("FUN_")) continue;
            Function fn = getFunctionAt(toAddr(f[0]));
            if (fn == null) fn = createFunction(toAddr(f[0]), f[1]);
            else fn.setName(f[1], SourceType.USER_DEFINED);
        }
    }
}
