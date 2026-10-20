mod common;
use common::*;

#[test]
fn scratch() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. S.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 A PIC 99V9(6) VALUE 22.222222.
01 B PIC 9(6)V9(6).
01 C REDEFINES B PIC S9(12).
PROCEDURE DIVISION.
M.
    MOVE A TO B.
    DISPLAY "B=" B.
    MULTIPLY 0.4 BY B ROUNDED.
    DISPLAY "B=" B " C=" C.
    STOP RUN.
"#;
    let o = run_wasm(src, &cobolt_wasm::Options::default());
    println!("WASM: {:?} errors={:?} trap={:?}", o.output, o.errors, o.trapped);
    println!("INTERP: {:?}", run_interpreter(src));
}
