      *> Working items of the PartiQL statements that the pq-*.cpy paragraphs run on the Backend's indexed files.
      *> WS-PQ-STATUS answers like a FILE STATUS: 00 done, 10 no more rows, 22 the key exists, 23 no such record.
      *> WS-PQ-ROW holds one fetched row; WS-PQ-NUMBER-n hold the values of a row that NUMVAL turns into numbers.
       01 WS-PQ-COUNT                            PIC    9(6).
       01 WS-PQ-STATUS                           PIC    XX.
       01 WS-PQ-ROW                              PIC    X(1000).
       01 WS-PQ-NUMBER-1                         PIC    X(20).
       01 WS-PQ-NUMBER-2                         PIC    X(20).
       01 WS-PQ-NUMBER-3                         PIC    X(20).
       01 WS-PQ-NUMBER-4                         PIC    X(20).
