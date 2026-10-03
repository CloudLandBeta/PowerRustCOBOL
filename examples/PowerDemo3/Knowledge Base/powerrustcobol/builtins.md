<!-- powerrustcobol-kit: 1.80.100 -->
# Built-ins — the `COBOL` object's methods

Every RustCOBOL built-in is written inline, as a method of the `COBOL` object, with its arguments in the order listed: data items are passed by reference, literals by content. It is a statement, never a value.

```cobol
           COBOL::"HTTP-GET" ( WS-URL WS-RESPONSE WS-HTTP-STATUS )
```

Each argument is marked `in` (the built-in reads it), `out` (it writes it) or `in-out`; one in brackets may be left out. This is the complete list.

| Built-in | Arguments | What it does |
|---|---|---|
| `COBOL::"APPEND-FILE"` | path in, text in, [status out] | Append a line of text to a file, creating it if missing |
| `COBOL::"BINDING-LOAD"` | binding-id in, status out | Load a data binding's records from its source |
| `COBOL::"BINDING-MARK-CLEAN"` | binding-id in, dirty-flag out | Mark a binding clean: no pending edits |
| `COBOL::"BINDING-POPULATE"` | binding-id in, status out | Fill a binding's bound controls from its data |
| `COBOL::"BINDING-SET-PENDING"` | binding-id in, row-key in, value in, dirty-flag out | Record a pending edit for one row of a binding |
| `COBOL::"BINDING-SET-READ-ONLY"` | binding-id in, flag in | Make a binding read-only (flag not "0") or writable |
| `COBOL::"BINDING-UPDATE"` | binding-id in, row-key in, status out | Write a binding row's pending changes back to its source |
| `COBOL::"CHART-ADD-POINT"` | chart-id in, label in, value in, [more values in] | Add one point to a chart |
| `COBOL::"CHART-CLEAR"` | chart-id in | Remove all data from a chart |
| `COBOL::"CHART-REFRESH"` | chart-id in | Repaint a chart from its current data |
| `COBOL::"CHART-SET-TABLE"` | chart-id in, table in, count in | Replace a chart's data with count rows of a table |
| `COBOL::"CLOSE-DB"` | handle in | Close a database connection |
| `COBOL::"EXEC-SQL"` | handle in, sql in, row-count out, status out | Run a SQL statement; returns the row or affected count |
| `COBOL::"FETCH-ROW"` | handle in, column in, value out, status out | Read one column (1-based) of the current result row |
| `COBOL::"FILE-STATUS"` | file-name in, status out | Copy a file's last FILE STATUS code into a data item |
| `COBOL::"FOLDER-DIALOG"` | title in, [start-folder in], path out | Ask the operator for a folder; spaces when cancelled |
| `COBOL::"GET-PROPERTY"` | object in, property in, value out | Read a control's property into a data item |
| `COBOL::"HTTP-CLEAR-HEADERS"` | — | Remove every header set with HTTP-SET-HEADER |
| `COBOL::"HTTP-DELETE"` | url in, response out, http-status out | Send an HTTP DELETE; returns the body and status code |
| `COBOL::"HTTP-GET"` | url in, response out, http-status out | Send an HTTP GET; returns the body and status code |
| `COBOL::"HTTP-POST"` | url in, body in, response out, http-status out | Send an HTTP POST (JSON by default); returns body and status |
| `COBOL::"HTTP-PUT"` | url in, body in, response out, http-status out | Send an HTTP PUT; returns the body and status code |
| `COBOL::"HTTP-SET-HEADER"` | name in, value in | Add or replace a header sent on every later HTTP call |
| `COBOL::"INIT-FORM"` | [form-name in] | Initialise the form (generated code calls it) |
| `COBOL::"KEY-IS-SET"` | entry in, flag out | Whether an API key is stored for a model entry: Y or N |
| `COBOL::"KEY-REMOVE"` | entry in, [status out] | Delete the stored API key of a model entry |
| `COBOL::"KEY-SET"` | entry in, key in, [status out] | Store the API key of a model entry (never read back) |
| `COBOL::"MCP-SEARCH"` | tool in, arguments-json in, result out | Call an MCP tool with JSON arguments; returns its text |
| `COBOL::"MODEL-LIST"` | provider in, endpoint in, key in, count out, status out, [entry in] | Ask a provider which models it offers |
| `COBOL::"MODEL-LIST-GET"` | index in, model out | One model name (1-based) from the last MODEL-LIST |
| `COBOL::"MODEL-REMOVE"` | entry in, [status out] | Remove an entry from the model list |
| `COBOL::"MODEL-SET"` | entry in, api in, url in, model in, [status out] | Add or replace an entry in the model list |
| `COBOL::"MODEL-TEST"` | provider in, endpoint in, model in, key in, [status out], [entry in] | Send a model a tiny request to test the connection |
| `COBOL::"NEXT-ROW"` | handle in, more out | Move to the next result row: Y when there is one, N at the end |
| `COBOL::"OPEN-DB"` | connection-string in, handle out, status out | Open a SQLite, PostgreSQL or MySQL connection |
| `COBOL::"OPEN-FILE-DIALOG"` | title in, [filter in], [start-folder in], path out | Ask the operator for a file to open; spaces when cancelled |
| `COBOL::"PROVIDER-COUNT"` | count out | How many model providers there are |
| `COBOL::"PROVIDER-GET"` | index in, [id out], [label out], [endpoint out], [needs-key out] | One provider's details (1-based) |
| `COBOL::"ROW-COUNT"` | handle in, count out | How many rows the last result set has |
| `COBOL::"SAVE-FILE-DIALOG"` | title in, [filter in], [file-name in], [start-folder in], path out | Ask the operator where to save; spaces when cancelled |
| `COBOL::"SET-PROPERTY"` | object in, property in, value in | Set a control's property |
| `COBOL::"WAIT-EVENT"` | event-id out, control-id out | Wait for the next event (generated code calls it) |
| `COBOL::"WRITE-FILE"` | path in, text in, [status out] | Write a file holding one line of text, replacing it |
