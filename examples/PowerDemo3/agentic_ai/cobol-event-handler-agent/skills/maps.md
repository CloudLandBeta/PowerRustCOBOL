# Maps Skill — building a real map solution

## The one thing to get right first

The Maps control is **two independent halves with different credential needs**:

| Half | Needs a key? | What it does |
|------|--------------|--------------|
| Basemap, markers, routes, regions | **No, never** | Draws OpenStreetMap tiles and whatever geometry the program holds |
| `Geocode`, `ReverseGeocode`, `Directions`, `DistanceMatrix`, `PlacesSearch` | **Yes** — Google Maps key in Settings → Integrations | Asks Google a question |

Build the drawing half FIRST. It works on a machine with nothing configured,
which is where most demos and most tests run. Never tell a developer they need
an API key to put a pin on a map, or to draw a route or a territory — they do
not.

## The data methods do NOT return the answer

All five are **asynchronous**. They return an EMPTY string immediately, set
`Busy` to `1`, and the answer arrives later on `onComplete` in `ResponseBody`.
There is no synchronous mode. This does not work, however much it reads like it
should:

```cobol
      *> WRONG — Directions returns "" here, always.
           MOVE MAP-1::Directions("Madrid", "Granada") TO WS-ANSWER
```

Write the call in one handler and the answer in `onComplete`:

```cobol
      *> In the button's onClick:
           INVOKE MAP-1 "Directions" USING "Madrid, Spain" "Granada, Spain"

      *> In the map's onComplete — SEVEN tab-separated fields:
      *>   text distance, text duration, summary, METRES, SECONDS, polyline,
      *>   and SECONDS WITH CURRENT TRAFFIC (0 when Google supplied none).
           UNSTRING MAP-1::ResponseBody DELIMITED BY X"09"
               INTO WS-DIST-TEXT WS-TIME-TEXT WS-SUMMARY
                    WS-METERS WS-SECONDS WS-POLYLINE WS-TRAFFIC-SECS
           COMPUTE WS-KM   = WS-METERS / 1000
           COMPUTE WS-COST = WS-KM * 0.62

      *> Prefer the traffic figure when there is one: it is the honest answer
      *> to "how long will this take, leaving now".
           IF WS-TRAFFIC-SECS > 0
               COMPUTE WS-MINUTES = WS-TRAFFIC-SECS / 60
           ELSE
               COMPUTE WS-MINUTES = WS-SECONDS / 60
           END-IF
```

Traffic is available as a NUMBER only. Google exposes its traffic *layer* through
its own JavaScript and mobile SDKs, never as map tiles, so there is no coloured
overlay to draw and asking for one is a dead end — but the drive time with
current traffic is right there in the last field, and a number is what a business
program can act on anyway.

`WS-METERS` and `WS-SECONDS` are the point. The text fields are for showing; the
numbers are what a business program computes with. Never parse `"72,4 km"` to
get a number back out of it — the number is already there, in the next field.

With no key configured the call fails on **`onError`** with `LastError`
explaining it. It never attempts a request, so handle `onError` and say what is
missing rather than leaving the form silent.

## Drawing: markers, routes, regions

Three collections, all the same shape — one TAB-separated record per line in a
string property — and all with the same rule: **re-using an id REPLACES that
record**, so a map that redraws itself as its data changes does not accumulate
invisible duplicates.

```cobol
      *> Pins. label shows on hover, info in the click card.
           INVOKE MAP-1 "AddMarker" USING
               "ANA" "40.4168" "-3.7038" "Ana - Centro" "27 accounts"

      *> A traced line. Geometry is EITHER an encoded polyline (exactly what
      *> Directions returned in its sixth field, so Google's own route traces
      *> with no conversion) OR an explicit lat,lng list you computed.
           INVOKE MAP-1 "AddRoute" USING "PLANNED" "#1E6EDC" "5"
               "40.4168,-3.7038;38.99,-3.37;37.1773,-3.5986"
           INVOKE MAP-1 "AddRoute" USING "DRIVEN" "#12A150" "6" WS-POLYLINE

      *> A filled territory. The fill takes an ALPHA (#RRGGBBAA) so the streets
      *> stay readable under it. The ring closes itself and MAY be concave.
           INVOKE MAP-1 "AddRegion" USING "NORTE" "#E5484D55" "#E5484D" "2"
               "43.79,-7.87;43.55,-5.66;42.60,-6.50;42.40,-8.87"
               "Norte - Elena" "18 accounts - 1.24M EUR YTD"
```

Also: `RemoveMarker`/`RemoveRoute`/`RemoveRegion` by id, and
`ClearRoutes`/`ClearRegions`.

## Positioning the view

`CenterLat`, `CenterLng` and `Zoom` are plain properties. Writing them moves the
map; the developer panning or zooming writes them back and fires
`onBoundsChanged`.

```cobol
           MOVE "40.0000" TO MAP-1::CenterLat
           MOVE "-3.7000" TO MAP-1::CenterLng
           MOVE 6         TO MAP-1::Zoom
```

Latitude and longitude are **strings**, not numerics — they carry more decimal
places than a PIC 9 would keep.

## The info window

Hovering a marker or region shows its `label`; clicking opens a card with the
`info` under it; clicking bare map closes the card. That is automatic — supply
`label`/`info` and it happens.

`SelectedMarkerId` / `SelectedRegionId` hold whichever card is open (write them
to open or close one from COBOL). `onMarkerHover`/`onRegionHover` fire beside
the native window, with `HoveredMarkerId`/`HoveredRegionId`, for a form that
wants to build its own panel or fetch something on hover.

Restyle it with `InfoBackgroundColor`, `InfoForegroundColor`,
`InfoBorderColor`, `InfoCornerRadius`, `InfoShadow`. Leave them EMPTY and the
window follows the form — which is the right default; do not set them unless
the developer asked for a specific look.

**Do not set `InfoForegroundColor` to "make it readable".** Left empty, the text
colour is derived from whichever background the window ended up with — black or
white, whichever contrasts more — so it is legible on any card. Setting it by
hand REPLACES that guarantee with your guess, which is how the window ended up
white-on-light in the first place.

## Checklist before claiming a map solution is done

- Does anything visual depend on an API key? It must not.
- Is every data-method result read in `onComplete`, never from the call?
- Is `onError` handled, so a missing key explains itself?
- Are distances computed from the METRES field, not parsed from text?
- Does redrawing re-use ids, so nothing accumulates?
