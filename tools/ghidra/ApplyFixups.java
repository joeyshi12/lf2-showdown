// Add call-fixups through a program specification extension, so the decompiler models
// two helpers exactly: abs_int (0x4034e0, preserves ECX and EDX) and _ftol2
// (0x4450d0, converts and pops ST0, result in EDX:EAX).
import ghidra.app.script.GhidraScript;
import ghidra.program.database.SpecExtension;
import ghidra.program.model.listing.Function;

public class ApplyFixups extends GhidraScript {
    @Override
    public void run() throws Exception {
        SpecExtension ext = new SpecExtension(currentProgram);
        // At the injection point the CALL has pushed its return address, and the
        // body must pop it as the callee's RET would.
        String abs = "<callfixup name=\"lf2_abs\"><pcode><body><![CDATA[\n"
            + " tmpp:4 = ESP + 4;\n EAX = *:4 tmpp;\n tmpabs:4 = EAX s>> 31;\n EAX = (EAX ^ tmpabs) - tmpabs;\n ESP = ESP + 4;\n"
            + "]]></body></pcode></callfixup>";
        String ftol = "<callfixup name=\"lf2_ftol\"><pcode><body><![CDATA[\n"
            + " tmpf:8 = trunc(ST0);\n EAX = tmpf:4;\n EDX = tmpf(4);\n"
            + " ST0 = ST1; ST1 = ST2; ST2 = ST3; ST3 = ST4; ST4 = ST5; ST5 = ST6; ST6 = ST7;\n ESP = ESP + 4;\n"
            + "]]></body></pcode></callfixup>";
        ext.addReplaceCompilerSpecExtension(abs, monitor);
        ext.addReplaceCompilerSpecExtension(ftol, monitor);
        Function a = getFunctionAt(toAddr(0x4034e0));
        a.setInline(false);
        a.setCallFixup("lf2_abs");
        getFunctionAt(toAddr(0x4450d0)).setCallFixup("lf2_ftol");
    }
}
