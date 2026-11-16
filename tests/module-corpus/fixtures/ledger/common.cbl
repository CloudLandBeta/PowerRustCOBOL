      *> Common Code of the "ledger" corpus fixture: the interest on a
      *> balance at a rate, rounded to the cent.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORPUSINTEREST.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-BALANCE   PIC S9(7)V99.
       01 LK-RATE      PIC 9V9999.
       01 LK-INTEREST  PIC S9(7)V99.
       PROCEDURE DIVISION USING LK-BALANCE LK-RATE LK-INTEREST.
           COMPUTE LK-INTEREST ROUNDED = LK-BALANCE * LK-RATE
           GOBACK.
