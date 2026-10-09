// Create structures from re/types.txt and apply function signatures from
// re/signatures.txt, so the decompile shows field names.
//
// types.txt:
//   struct NAME SIZE
//   OFFSET TYPE FIELD        (TYPE: int uint short ushort char uchar byte double float
//                             undefined4, a struct name, any of these followed by '*',
//                             or by [N] for an array)
//   end
// Structs are created first (empty, with their size) so that they can refer to each
// other in any order. Lines starting with '#' are comments.
//
// signatures.txt: ADDRESS<TAB>C prototype, e.g.
//   0040e490	void __fastcall obj_move(Object * o)
import ghidra.app.script.GhidraScript;
import ghidra.app.cmd.function.ApplyFunctionSignatureCmd;
import ghidra.app.util.parser.FunctionSignatureParser;
import ghidra.program.model.data.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.SourceType;
import java.nio.file.*;
import java.util.*;

public class ApplyTypes extends GhidraScript {
    DataTypeManager dtm;
    Map<String, StructureDataType> structs = new LinkedHashMap<>();
    CategoryPath cat = new CategoryPath("/lf2");

    DataType base(String t) {
        switch (t) {
            case "int": return IntegerDataType.dataType;
            case "uint": return UnsignedIntegerDataType.dataType;
            case "short": return ShortDataType.dataType;
            case "ushort": return UnsignedShortDataType.dataType;
            case "char": return CharDataType.dataType;
            case "uchar": case "byte": return ByteDataType.dataType;
            case "double": return DoubleDataType.dataType;
            case "float": return FloatDataType.dataType;
            case "undefined4": return Undefined4DataType.dataType;
            case "void": return VoidDataType.dataType;
        }
        DataType d = dtm.getDataType(cat, t);
        if (d == null) throw new RuntimeException("unknown type " + t);
        return d;
    }

    DataType parse(String t) {
        t = t.trim();
        if (t.endsWith("*")) return new PointerDataType(parse(t.substring(0, t.length() - 1)), dtm);
        if (t.endsWith("]")) {
            int i = t.lastIndexOf('[');
            int n = Integer.decode(t.substring(i + 1, t.length() - 1));
            DataType e = parse(t.substring(0, i));
            return new ArrayDataType(e, n, e.getLength(), dtm);
        }
        return base(t);
    }

    @Override
    public void run() throws Exception {
        String[] a = getScriptArgs();
        dtm = currentProgram.getDataTypeManager();
        Path tp = Paths.get(a[0]);
        if (Files.exists(tp)) {
            List<String> lines = Files.readAllLines(tp);
            int tx = dtm.startTransaction("types");
            for (String l : lines) {
                String[] f = l.trim().split("\\s+");
                if (f.length == 3 && f[0].equals("struct")) {
                    StructureDataType s = new StructureDataType(cat, f[1], Integer.decode(f[2]), dtm);
                    dtm.addDataType(s, DataTypeConflictHandler.REPLACE_HANDLER);
                }
            }
            Structure cur = null;
            for (String l : lines) {
                l = l.trim();
                if (l.isEmpty() || l.startsWith("#")) continue;
                String[] f = l.split("\\s+");
                if (f[0].equals("struct")) { cur = (Structure) dtm.getDataType(cat, f[1]); continue; }
                if (f[0].equals("end")) { cur = null; continue; }
                if (cur == null) continue;
                int off = Integer.decode(f[0]);
                DataType dt = parse(f[1]);
                try {
                    cur.replaceAtOffset(off, dt, dt.getLength(), f[2], null);
                } catch (Exception e) {
                    println("field " + l + ": " + e);
                }
            }
            dtm.endTransaction(tx, true);
        }
        Path sp = Paths.get(a[1]);
        if (Files.exists(sp)) {
            FunctionSignatureParser parser = new FunctionSignatureParser(dtm, null);
            for (String l : Files.readAllLines(sp)) {
                if (l.startsWith("#") || l.trim().isEmpty()) continue;
                String[] f = l.split("\t");
                if (f[0].equals("INLINE")) {
                    Function fi = getFunctionAt(toAddr(f[1]));
                    if (fi != null) fi.setInline(true);
                    continue;
                }
                if (f[0].equals("NOINLINE")) {
                    Function fi = getFunctionAt(toAddr(f[1]));
                    if (fi != null) fi.setInline(false);
                    continue;
                }
                Function fn = getFunctionAt(toAddr(f[0]));
                if (fn == null) fn = createFunction(toAddr(f[0]), null);
                if (fn == null) { println("no function at " + f[0]); continue; }
                String proto = f[1];
                String conv = null;
                for (String c : new String[] {"__thiscall", "__fastcall", "__cdecl", "__stdcall"}) {
                    if (proto.contains(c)) { conv = c; proto = proto.replace(c, " "); }
                }
                // A thiscall prototype written with an explicit "T * this" first
                // parameter: apply it without that parameter, then give the automatic
                // `this` the type T through custom storage.
                String thisType = null;
                java.util.regex.Matcher m = java.util.regex.Pattern
                    .compile("\\(\\s*(\\w+)\\s*\\*\\s*this\\s*(,|\\))").matcher(proto);
                if ("__thiscall".equals(conv) && m.find()) {
                    thisType = m.group(1);
                    proto = proto.substring(0, m.start()) + "(" + (m.group(2).equals(")") ? ")" : "") + proto.substring(m.end());
                }
                try {
                    FunctionDefinitionDataType sig = parser.parse(fn.getSignature(), proto);
                    ApplyFunctionSignatureCmd cmd = new ApplyFunctionSignatureCmd(fn.getEntryPoint(), sig, SourceType.USER_DEFINED);
                    if (!cmd.applyTo(currentProgram)) println("signature " + f[0] + ": " + cmd.getStatusMsg());
                    if (conv != null) fn.setCallingConvention(conv);
                    if (thisType != null) {
                        fn.setCustomVariableStorage(true);
                        fn.getParameter(0).setDataType(new PointerDataType(base(thisType), dtm), SourceType.USER_DEFINED);
                        fn.getParameter(0).setName("this", SourceType.USER_DEFINED);
                    }
                } catch (Exception e) {
                    println("signature " + l + ": " + e);
                }
            }
        }
    }
}
