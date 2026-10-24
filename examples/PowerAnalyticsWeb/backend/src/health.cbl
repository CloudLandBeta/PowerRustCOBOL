       IDENTIFICATION DIVISION.
       PROGRAM-ID. HEALTH.
      *> PowerAnalyticsWeb.project — Backend health check. A caller that is allowed to ask
      *> gets OK back; it is the first program in backend/exports.toml.

       DATA DIVISION.

       LINKAGE SECTION.
       01 LK-STATUS                              PIC    X(8).

       PROCEDURE DIVISION USING LK-STATUS.
           MOVE    "OK"                          TO    LK-STATUS
           GOBACK.
