# PowerOficina data

These sixteen `.idx` files are the application's indexed files, filled with **fictitious** data so that
every screen and the Dashboard have something to show the first time the example is opened:

- the catalog (parts, services, add-ons, the labour rate), the fault rules and the number sequences,
  made by `backend/src/seed-load.cbl`;
- 14 customers, 16 vehicles and 39 quotes over six months, in BRL and EUR, in every state a quote reaches,
  with their diagnoses, approvals, payments, instalments, the queue and the messages, made by
  `tests/demo-data.cbl` through the Backend programs themselves.

The names, documents, plates, chassis numbers, phones and addresses are invented; the approval links point
at `oficina.exemplo`, which does not exist.

The Dashboard counts the months back from today, so as the weeks pass the older data fills fewer of them.
To get a fresh set, empty the folder (keep this file) and run the two programs above again, as their headers
say: `seed-load` first, then `demo-data`, once, on files that hold no other demo data.

The `.lck` files an open application leaves beside them are not part of the example.
