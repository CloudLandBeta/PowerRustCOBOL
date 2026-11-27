IDENTIFICATION DIVISION.
PROGRAM-ID. FAILER.
DATA DIVISION.
LINKAGE SECTION.
01 LK.
    05 CODE-IN PIC 9(5).
    05 NOTE    PIC X(30).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    MOVE "changed by the program" TO NOTE
    COBOL::"WS-FAIL" ( CODE-IN "declined by the bank   " "retry=no" )
    MOVE "after the failure" TO NOTE
    GOBACK.
