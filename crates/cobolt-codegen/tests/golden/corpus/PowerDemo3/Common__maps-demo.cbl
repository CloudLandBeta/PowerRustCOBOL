      *> ───────────────────────────────────────────────────────────
      *>  This code was generated automatically by PowerRustCOBOL RAD.
      *>
      *>  DO NOT MODIFY IT DIRECTLY: it is regenerated the next time
      *>  you interact with the Form Designer, so manual edits are lost.
      *>  Edit the form and its event handlers in the Form Designer
      *>  instead.
      *>
      *>  PowerRustCOBOL may change the structure of this generated code
      *>  at any time — without breaking your code's functionality — for
      *>  reasons such as performance improvements, new observability
      *>  features, and bug fixes.
      *>
      *>  PowerRustCOBOL and its components are distributed under the
      *>  Apache 2.0 License.
      *> ───────────────────────────────────────────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAPS-DEMO.

       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-BOOL IS "Rust.bool"
           CLASS RUST-CHAR IS "Rust.char"
           CLASS RUST-I8 IS "Rust.i8"
           CLASS RUST-I16 IS "Rust.i16"
           CLASS RUST-I32 IS "Rust.i32"
           CLASS RUST-I64 IS "Rust.i64"
           CLASS RUST-I128 IS "Rust.i128"
           CLASS RUST-ISIZE IS "Rust.isize"
           CLASS RUST-U8 IS "Rust.u8"
           CLASS RUST-U16 IS "Rust.u16"
           CLASS RUST-U32 IS "Rust.u32"
           CLASS RUST-U64 IS "Rust.u64"
           CLASS RUST-U128 IS "Rust.u128"
           CLASS RUST-USIZE IS "Rust.usize"
           CLASS RUST-F32 IS "Rust.f32"
           CLASS RUST-F64 IS "Rust.f64"
           CLASS RUST-STR IS "Rust.str"
           CLASS RUST-UNIT IS "Rust.unit"
           CLASS RUST-STRING IS "Rust.String"
           CLASS RUST-OSSTRING IS "Rust.OsString"
           CLASS RUST-OSSTR IS "Rust.OsStr"
           CLASS RUST-CSTRING IS "Rust.CString"
           CLASS RUST-CSTR IS "Rust.CStr"
           CLASS RUST-PATH IS "Rust.Path"
           CLASS RUST-PATHBUF IS "Rust.PathBuf"
           CLASS RUST-VEC IS "Rust.Vec"
           CLASS RUST-VECDEQUE IS "Rust.VecDeque"
           CLASS RUST-LINKEDLIST IS "Rust.LinkedList"
           CLASS RUST-HASHMAP IS "Rust.HashMap"
           CLASS RUST-BTREEMAP IS "Rust.BTreeMap"
           CLASS RUST-HASHSET IS "Rust.HashSet"
           CLASS RUST-BTREESET IS "Rust.BTreeSet"
           CLASS RUST-BINARYHEAP IS "Rust.BinaryHeap"
           CLASS RUST-OPTION IS "Rust.Option"
           CLASS RUST-RESULT IS "Rust.Result"
           CLASS RUST-BOX IS "Rust.Box"
           CLASS RUST-RC IS "Rust.Rc"
           CLASS RUST-ARC IS "Rust.Arc"
           CLASS RUST-WEAK IS "Rust.Weak"
           CLASS RUST-CELL IS "Rust.Cell"
           CLASS RUST-REFCELL IS "Rust.RefCell"
           CLASS RUST-MUTEX IS "Rust.Mutex"
           CLASS RUST-RWLOCK IS "Rust.RwLock"
           CLASS RUST-COW IS "Rust.Cow"
           CLASS RUST-DURATION IS "Rust.Duration"
           CLASS RUST-INSTANT IS "Rust.Instant"
           CLASS RUST-SYSTEMTIME IS "Rust.SystemTime"
           CLASS RUST-RANGE IS "Rust.Range".

       DATA DIVISION.
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'MAPS-DEMO'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-DIST-TEXT GLOBAL  PIC X(40) VALUE SPACES.
       01 WS-TIME-TEXT GLOBAL  PIC X(40) VALUE SPACES.
       01 WS-SUMMARY GLOBAL    PIC X(80) VALUE SPACES.
       01 WS-METERS GLOBAL     PIC 9(9)  VALUE 0.
       01 WS-SECONDS GLOBAL    PIC 9(9)  VALUE 0.
      *> Field 6 of a Directions answer is the road, turn by turn, and never
      *> exceeds 4,000 characters — so 4096 holds any route Google returns.
      *> Size this too small and the geometry truncates: the map then draws a
      *> route that stops partway and gives no hint why.
       01 WS-POLYLINE GLOBAL  PIC X(4096) VALUE SPACES.
      *> How many characters of geometry came back (see MAP-1 onComplete).
      *> A TALLYING counter must be plain numeric, so the edited copy that is
      *> fit to show a human is a second item.
       01 WS-GEOM-LEN GLOBAL    PIC 9(4)  VALUE 0.
       01 WS-GEOM-SHOWN GLOBAL  PIC Z(3)9 VALUE ZERO.
      *> Which routing service was asked, so the one onComplete event knows
      *> which answer shape it is holding: "G" = Google Directions (seven
      *> fields), "O" = OpenRouteService TraceRoad (three).
       01 WS-PROVIDER GLOBAL  PIC X VALUE SPACE.
      *> The operator's OpenRouteService key, read from the form when the
      *> button is pressed and never written anywhere else.
       01 WS-ORS-KEY GLOBAL       PIC X(512)  VALUE SPACES.
       01 WS-TRAFFIC-SECS GLOBAL  PIC 9(9)    VALUE 0.
       01 WS-KM GLOBAL            PIC 9(6)V99 VALUE 0.
       01 WS-MINUTES GLOBAL       PIC 9(6)    VALUE 0.
       01 WS-COST GLOBAL          PIC 9(7)V99 VALUE 0.
       01 WS-MSG GLOBAL           PIC X(120)  VALUE SPACES.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-LBL-TITLE.
          05 WS-LBL-TITLE-TEXT       PIC X(256) VALUE 'Maps — markers, traced routes, sales regions and drive times'.
          05 WS-LBL-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-MAP-1.
          05 WS-MAP-1-TEXT       PIC X(256) VALUE 'MAP-1'.
          05 WS-MAP-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-MAP-1-ENABLED    PIC 9      VALUE 1.
          05 WS-MAP-1-CENTER-LAT PIC X(32)  VALUE '40.0000'.
          05 WS-MAP-1-CENTER-LNG PIC X(32)  VALUE '-3.7000'.
          05 WS-MAP-1-ZOOM       PIC S9(4)  VALUE 6.

       01 WS-BTN-MARKERS.
          05 WS-BTN-MARKERS-TEXT       PIC X(256) VALUE '1 — Place the five salesmen'.
          05 WS-BTN-MARKERS-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-MARKERS-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-REGIONS.
          05 WS-BTN-REGIONS-TEXT       PIC X(256) VALUE '2 — Draw the five sales regions'.
          05 WS-BTN-REGIONS-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-REGIONS-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-ROUTE.
          05 WS-BTN-ROUTE-TEXT       PIC X(256) VALUE '3 — Planned corridor (no key)'.
          05 WS-BTN-ROUTE-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-ROUTE-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-DRIVE.
          05 WS-BTN-DRIVE-TEXT       PIC X(256) VALUE '4 — Real road + drive time (needs key)'.
          05 WS-BTN-DRIVE-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-DRIVE-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-CLEAR.
          05 WS-BTN-CLEAR-TEXT       PIC X(256) VALUE '5 — Clear everything'.
          05 WS-BTN-CLEAR-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-CLEAR-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-NUMBERS.
          05 WS-LBL-NUMBERS-TEXT       PIC X(256) VALUE 'Drive time, as numbers you can compute with'.
          05 WS-LBL-NUMBERS-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-NUMBERS-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-KM-CAP.
          05 WS-LBL-KM-CAP-TEXT       PIC X(256) VALUE 'Kilometres'.
          05 WS-LBL-KM-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-KM-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-KM.
          05 WS-LBL-KM-TEXT       PIC X(256) VALUE 'LBL-KM'.
          05 WS-LBL-KM-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-KM-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-MIN-CAP.
          05 WS-LBL-MIN-CAP-TEXT       PIC X(256) VALUE 'Minutes'.
          05 WS-LBL-MIN-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-MIN-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-MIN.
          05 WS-LBL-MIN-TEXT       PIC X(256) VALUE 'LBL-MIN'.
          05 WS-LBL-MIN-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-MIN-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-COST-CAP.
          05 WS-LBL-COST-CAP-TEXT       PIC X(256) VALUE 'Cost @ 0.62/km'.
          05 WS-LBL-COST-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-COST-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-COST.
          05 WS-LBL-COST-TEXT       PIC X(256) VALUE 'LBL-COST'.
          05 WS-LBL-COST-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-COST-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-STATUS.
          05 WS-LBL-STATUS-TEXT       PIC X(256) VALUE 'Ready.'.
          05 WS-LBL-STATUS-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-STATUS-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-HINT.
          05 WS-LBL-HINT-TEXT       PIC X(256) VALUE 'Buttons 1-3 and 5 need no API key: the basemap is OpenStreetMap and the geometry comes from this program. Buttons 4 and 6 ask a routing service for the real road. Drag to pan; the wheel zooms about the pointer.'.
          05 WS-LBL-HINT-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-HINT-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-ORS-CAP.
          05 WS-LBL-ORS-CAP-TEXT       PIC X(256) VALUE 'Your OpenRouteService key (not saved):'.
          05 WS-LBL-ORS-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-ORS-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-ROAD.
          05 WS-BTN-ROAD-TEXT       PIC X(256) VALUE '6 — Real road, no Google (your ORS key)'.
          05 WS-BTN-ROAD-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-ROAD-ENABLED    PIC 9      VALUE 1.

       01 WS-TXT-ORS-KEY.
          05 WS-TXT-ORS-KEY-TEXT       PIC X(256) VALUE 'TXT-ORS-KEY'.
          05 WS-TXT-ORS-KEY-VISIBLE    PIC 9      VALUE 1.
          05 WS-TXT-ORS-KEY-ENABLED    PIC 9      VALUE 1.
          05 WS-TXT-ORS-KEY-VALUE      PIC X(256) VALUE SPACES.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "MAPS-DEMO--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "MAPS-DEMO--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "MAP-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMapClick"
                               CALL "MAP-1--MAPCLICK"
                           WHEN "onMarkerClick"
                               CALL "MAP-1--MARKERCLICK"
                           WHEN "onComplete"
                               CALL "MAP-1--COMPLETE"
                           WHEN "onError"
                               CALL "MAP-1--ERROR"
                       END-EVALUATE
                   WHEN "BTN-MARKERS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-MARKERS--CLICK"
                       END-EVALUATE
                   WHEN "BTN-REGIONS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-REGIONS--CLICK"
                       END-EVALUATE
                   WHEN "BTN-ROUTE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ROUTE--CLICK"
                       END-EVALUATE
                   WHEN "BTN-DRIVE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-DRIVE--CLICK"
                       END-EVALUATE
                   WHEN "BTN-CLEAR"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CLEAR--CLICK"
                       END-EVALUATE
                   WHEN "BTN-ROAD"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ROAD--CLICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
      *> </TIMER-STUBS>
      *> <CSV-EXPORT>
      *> </CSV-EXPORT>
      *> <REST-CLIENT>
      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>

      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAPS-DEMO--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Centre on Spain. The basemap is OpenStreetMap and needs no API key,
      *> so everything drawn below works with nothing configured.
           MOVE "40.0000" TO MAP-1::CenterLat
           MOVE "-3.7000" TO MAP-1::CenterLng
           MOVE 6         TO MAP-1::Zoom
           MOVE "Ready. Every button except Drive time works without an API key."
             TO LBL-STATUS::Caption

           GOBACK.

       END PROGRAM MAPS-DEMO--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAPS-DEMO--ONCLOSE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           CONTINUE.

           GOBACK.

       END PROGRAM MAPS-DEMO--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAP-1--MAPCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Every click reports where it landed, so the projection can be seen
      *> to be correct rather than taken on trust.
           MOVE SPACES TO WS-MSG
           STRING "Clicked at " DELIMITED BY SIZE
                  MAP-1::ClickLat DELIMITED BY SIZE
                  ", " DELIMITED BY SIZE
                  MAP-1::ClickLng DELIMITED BY SIZE
             INTO WS-MSG
           MOVE WS-MSG TO LBL-STATUS::Caption

           GOBACK.

       END PROGRAM MAP-1--MAPCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAP-1--MARKERCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE SPACES TO WS-MSG
           STRING "Salesman: " DELIMITED BY SIZE
                  MAP-1::SelectedMarkerId DELIMITED BY SIZE
             INTO WS-MSG
           MOVE WS-MSG TO LBL-STATUS::Caption

           GOBACK.

       END PROGRAM MAP-1--MARKERCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAP-1--COMPLETE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> BOTH routing services answer on this one event, and they do not answer
      *> in the same shape — Google sends seven TAB-separated fields,
      *> OpenRouteService sends three. WS-PROVIDER was set by whichever button
      *> made the call, which is how one handler serves both without guessing.
           IF WS-PROVIDER = "O"
               PERFORM ORS-ANSWER
               GOBACK
           END-IF

      *> Directions answers here, never from the call itself. ResponseBody is
      *> SEVEN TAB-separated fields:
      *>   distance text, duration text, summary, METRES, SECONDS, polyline,
      *>   and SECONDS WITH CURRENT TRAFFIC.
      *> The first three are for reading; the numbers are for computing with;
      *> the polyline is the route itself, ready for AddRoute.
      *>
      *> Field 6 is the ROAD, turn by turn — not the thumbnail-grade summary
      *> Google also publishes — so the green line sits ON the motorway rather
      *> than near it. It never exceeds 4,000 characters, which is why
      *> WS-POLYLINE is PIC X(4096): a geometry field too small to hold the
      *> answer truncates it, and a truncated polyline draws a route that
      *> stops in the middle of nowhere.
           UNSTRING MAP-1::ResponseBody DELIMITED BY X"09"
               INTO WS-DIST-TEXT
                    WS-TIME-TEXT
                    WS-SUMMARY
                    WS-METERS
                    WS-SECONDS
                    WS-POLYLINE
                    WS-TRAFFIC-SECS

      *> Trace the real road, in green, over the planned corridor from button 3.
      *> Compare the two: the blue line is eight points you wrote, the green one
      *> is every bend Google's route actually takes.
           INVOKE MAP-1 "AddRoute" USING
               "DRIVEN" "#12A150" "6" WS-POLYLINE

      *> How much road geometry came back, in characters. It is the one number
      *> that tells the two lines apart at a glance: the corridor above is a
      *> single short literal, this is thousands of characters of road. The
      *> encoding uses no spaces, so counting up to the first one measures it.
           MOVE 0 TO WS-GEOM-LEN
           INSPECT WS-POLYLINE TALLYING WS-GEOM-LEN
               FOR CHARACTERS BEFORE INITIAL SPACE
           MOVE WS-GEOM-LEN TO WS-GEOM-SHOWN

      *> The numbers are the point: a distance you can only print cannot be
      *> charged for. 0.62 EUR per km, say.
           COMPUTE WS-KM   = WS-METERS / 1000
           COMPUTE WS-COST = WS-KM * 0.62

      *> Prefer the traffic figure when Google supplied one — it is the honest
      *> answer to "how long will this take, leaving now". There is no traffic
      *> OVERLAY to draw (Google gives that only through its own SDKs), but the
      *> number is right here, and a number is what we can act on.
           IF WS-TRAFFIC-SECS > 0
               COMPUTE WS-MINUTES = WS-TRAFFIC-SECS / 60
           ELSE
               COMPUTE WS-MINUTES = WS-SECONDS / 60
           END-IF

           MOVE SPACES TO WS-MSG

           STRING "Madrid to Granada: "
                  WS-DIST-TEXT DELIMITED BY SPACE
                  " / "
                  WS-TIME-TEXT DELIMITED BY SPACE
                  "  via "
                  WS-SUMMARY DELIMITED BY SPACE
                  "  - road traced from "
                  WS-GEOM-SHOWN
                  " chars of geometry"
             INTO WS-MSG
           MOVE WS-MSG TO LBL-STATUS::Caption

           MOVE WS-KM      TO LBL-KM::Caption
           MOVE WS-MINUTES TO LBL-MIN::Caption
           MOVE WS-COST    TO LBL-COST::Caption

           GOBACK.

      *> OpenRouteService: THREE TAB-separated fields — metres, seconds, and
      *> the road geometry. No drive time with traffic and no route summary;
      *> what it does give is the road itself, without a Google credential.
      *> Drawn in orange, so all three lines can be told apart at a glance:
      *> blue = the corridor you wrote, green = Google's road, orange = ORS's.
       ORS-ANSWER.
           UNSTRING MAP-1::ResponseBody DELIMITED BY X"09"
               INTO WS-METERS
                    WS-SECONDS
                    WS-POLYLINE

           INVOKE MAP-1 "AddRoute" USING
               "ROAD" "#E8711A" "6" WS-POLYLINE

           MOVE 0 TO WS-GEOM-LEN
           INSPECT WS-POLYLINE TALLYING WS-GEOM-LEN
               FOR CHARACTERS BEFORE INITIAL SPACE
           MOVE WS-GEOM-LEN TO WS-GEOM-SHOWN

           COMPUTE WS-KM      = WS-METERS / 1000
           COMPUTE WS-COST    = WS-KM * 0.62
           COMPUTE WS-MINUTES = WS-SECONDS / 60

           MOVE SPACES TO WS-MSG
           STRING "OpenRouteService, no Google key: "
                  WS-GEOM-SHOWN
                  " chars of road geometry traced in orange."
             INTO WS-MSG
           MOVE WS-MSG TO LBL-STATUS::Caption

           MOVE WS-KM      TO LBL-KM::Caption
           MOVE WS-MINUTES TO LBL-MIN::Caption
           MOVE WS-COST    TO LBL-COST::Caption

           GOBACK.

       END PROGRAM MAP-1--COMPLETE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAP-1--ERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> With no Google Maps API key configured, the five data methods fail
      *> here instead of attempting a call. Everything drawn locally is
      *> unaffected — that half needs no credential at all.
           MOVE SPACES TO WS-MSG
           STRING "Drive time unavailable: " DELIMITED BY SIZE
                  MAP-1::LastError DELIMITED BY SIZE
             INTO WS-MSG
           MOVE WS-MSG TO LBL-STATUS::Caption

           GOBACK.

       END PROGRAM MAP-1--ERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-MARKERS--CLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.

       WORKING-STORAGE SECTION.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Invokes MAP-1 "AddMarker" to add a marker with id="ANA", latitude="40.4168", longitude="-3.7038", label="Ana — Centro", info="Madrid".
           INVOKE MAP-1 "AddMarker" USING "ANA" "40.4168" "-3.7038" "Ana — Centro" "Madrid"

           *> Invokes MAP-1 "AddMarker" to add a marker with id="BRUNO", latitude="41.3874", longitude="2.1686", label="Bruno — Noreste", info="Barcelona".
           INVOKE MAP-1 "AddMarker" USING "BRUNO" "41.3874" "2.1686" "Bruno — Noreste" "Barcelona"

           *> Invokes MAP-1 "AddMarker" to add a marker with id="CARMEN", latitude="39.4699", longitude="-0.3763", label="Carmen — Levante", info="Valencia".
           INVOKE MAP-1 "AddMarker" USING "CARMEN" "39.4699" "-0.3763" "Carmen — Levante" "Valencia"

           *> Invokes MAP-1 "AddMarker" to add a marker with id="DIEGO", latitude="37.3891", longitude="-5.9845", label="Diego — Sur", info="Sevilla".
           INVOKE MAP-1 "AddMarker" USING "DIEGO" "37.3891" "-5.9845" "Diego — Sur" "Sevilla"

           *> Invokes MAP-1 "AddMarker" to add a marker with id="ELENA", latitude="43.3623", longitude="-8.4115", label="Elena — Norte", info="A Coruna".
           INVOKE MAP-1 "AddMarker" USING "ELENA" "43.3623" "-8.4115" "Elena — Norte" "A Coruna"

           MOVE "Five markers placed. Click one." TO LBL-STATUS::Caption

           GOBACK.

       END PROGRAM BTN-MARKERS--CLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-REGIONS--CLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> AddRegion(id, fill, stroke, width, geometry, label, info).
      *>
      *> The fill carries its own alpha (#RRGGBBAA) so the streets stay
      *> readable underneath. Geometry is "lat,lng;lat,lng;..." — and it may
      *> be CONCAVE: the fill is triangulated, not assumed convex, which is
      *> what a real territory following coastlines and borders needs.
      *>
      *> The last two arguments are the info window: hovering a territory
      *> shows the LABEL, clicking it opens a card with the INFO underneath.
      *> Both are optional — leave them off and the region simply has no card.
           INVOKE MAP-1 "AddRegion" USING "NORTE" "#E5484D55" "#E5484D" "2"
               "43.79,-7.87;43.55,-5.66;43.40,-3.02;42.85,-2.95;42.60,-6.50;42.40,-8.87"
               "Norte - Elena" "18 accounts - 1.24M EUR YTD - A Coruna"

           INVOKE MAP-1 "AddRegion" USING "NORESTE" "#3E63DD55" "#3E63DD" "2"
               "42.85,-1.65;43.38,-1.79;42.50,3.17;41.10,1.20;40.30,-0.40;41.65,-1.90"
               "Noreste - Bruno" "31 accounts - 2.80M EUR YTD - Barcelona"

           INVOKE MAP-1 "AddRegion" USING "CENTRO" "#F5A62355" "#F5A623" "2"
               "42.00,-6.90;42.05,-2.30;40.20,-1.70;39.30,-2.60;38.90,-5.30;40.30,-7.00"
               "Centro - Ana" "27 accounts - 2.15M EUR YTD - Madrid"

           INVOKE MAP-1 "AddRegion" USING "LEVANTE" "#12A15055" "#12A150" "2"
               "40.55,-0.35;40.10,0.55;38.75,0.20;37.55,-0.70;37.40,-1.90;38.80,-1.35"
               "Levante - Carmen" "22 accounts - 1.63M EUR YTD - Valencia"

           INVOKE MAP-1 "AddRegion" USING "SUR" "#8E4EC655" "#8E4EC6" "2"
               "38.70,-6.30;38.30,-2.90;37.35,-1.65;36.72,-4.42;36.00,-5.60;37.20,-7.40"
               "Sur - Diego" "19 accounts - 1.41M EUR YTD - Sevilla"

           MOVE "Five territories. Hover one for its name, click for the detail."
             TO LBL-STATUS::Caption

           GOBACK.

       END PROGRAM BTN-REGIONS--CLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ROUTE--CLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> AddRoute(id, colour, width, geometry).
      *>
      *> A PLANNED CORRIDOR, not a traced road — and the difference is the
      *> lesson. The geometry below is EIGHT waypoints written by hand down the
      *> A-4/A-44, so this button needs no credential and works offline; the map
      *> draws every point given to it and invents none, which is why eight
      *> points cut every curve between them. Zoom in and the blue line leaves
      *> the tarmac. That is not a fault to fix here: no setting turns a sparse
      *> waypoint list into a road.
      *>
      *> Press button 4 next. It asks Google and draws the ROAD over this, in
      *> green, following it turn by turn — the two lines side by side are what
      *> "geometry you wrote" versus "geometry a routing service returned"
      *> actually looks like.
           INVOKE MAP-1 "AddRoute" USING "PLANNED" "#1E6EDC" "5"
               "40.4168,-3.7038;40.0300,-3.6000;39.4600,-3.5300;38.9900,-3.3700;38.7600,-3.3800;38.1000,-3.7700;37.7700,-3.7900;37.1773,-3.5986"

           INVOKE MAP-1 "AddMarker" USING
               "ORIGEN"  "40.4168" "-3.7038" "Madrid"  "Origin"
           INVOKE MAP-1 "AddMarker" USING
               "DESTINO" "37.1773" "-3.5986" "Granada" "Destination"

           MOVE "37.9000" TO MAP-1::CenterLat
           MOVE "-3.6500" TO MAP-1::CenterLng
           MOVE 7         TO MAP-1::Zoom
           MOVE "Planned corridor: 8 hand-written waypoints, no key. It cuts the curves - button 4 draws the real road over it."
             TO LBL-STATUS::Caption

           GOBACK.

       END PROGRAM BTN-ROUTE--CLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-DRIVE--CLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.

       WORKING-STORAGE SECTION.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> ASYNCHRONOUS. This returns immediately with an empty string and sets
      *> Busy to 1; the answer arrives on onComplete, above. Needs the Google
      *> Maps API key in Settings -> Integrations; without one it fails on
      *> onError instead of attempting a call.
           MOVE "G" TO WS-PROVIDER
           MOVE "Asking Google for the drive time..." TO LBL-STATUS::Caption

           INVOKE MAP-1 "Directions" USING "Madrid, Spain" "Granada, Spain"

           GOBACK.

       END PROGRAM BTN-DRIVE--CLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CLEAR--CLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           INVOKE MAP-1 "ClearRoutes"
           INVOKE MAP-1 "ClearRegions"
           MOVE SPACES TO MAP-1::Markers
           MOVE "40.0000" TO MAP-1::CenterLat
           MOVE "-3.7000" TO MAP-1::CenterLng
           MOVE 6         TO MAP-1::Zoom
           MOVE SPACES TO LBL-KM::Caption
           MOVE SPACES TO LBL-MIN::Caption
           MOVE SPACES TO LBL-COST::Caption
           MOVE "Cleared." TO LBL-STATUS::Caption

           GOBACK.

       END PROGRAM BTN-CLEAR--CLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ROAD--CLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> TraceRoad(key, fromLat, fromLng, toLat, toLng) — the road from
      *> OpenRouteService, with NO Google credential involved.
      *>
      *> THE KEY IS AN ARGUMENT, and it comes from the field the operator typed
      *> it into. PowerRustCOBOL does not store it: it is not in this form, not
      *> in the project manifest, not in any file on disk. Read it, pass it,
      *> forget it. (When local and cloud vaults arrive, that becomes the
      *> better place to keep one — this stays the way to work without a vault.)
      *>
      *> ASYNCHRONOUS like every other Maps data method: this returns an empty
      *> string at once and sets Busy; the answer arrives on onComplete, which
      *> tells the two kinds of answer apart by how many fields it holds.
           MOVE TXT-ORS-KEY::Text TO WS-ORS-KEY

           IF WS-ORS-KEY = SPACES
               MOVE "Paste an OpenRouteService key in the box first - it is never saved anywhere."
                 TO LBL-STATUS::Caption
           ELSE
      *>       Which provider is answering, so onComplete knows how many
      *>       fields to expect. Two services, two answer shapes, one event.
               MOVE "O" TO WS-PROVIDER
               MOVE "Asking OpenRouteService for the road..."
                 TO LBL-STATUS::Caption
               INVOKE MAP-1 "TraceRoad" USING
                   WS-ORS-KEY "40.4168" "-3.7038" "37.1773" "-3.5986"
           END-IF

           GOBACK.

       END PROGRAM BTN-ROAD--CLICK.

       END PROGRAM MAPS-DEMO.
