       IDENTIFICATION DIVISION.
       PROGRAM-ID. CRAFT-TILE.

      *> Paints one 16 x 16 picture, pixel by pixel, into LK-PIXELS (row by row from the top-left,
      *> 256 colours written "#RRGGBBAA"). The block faces are original pixel art made here from
      *> a few shades and a seeded random sequence, so every run draws the same world:
      *>   1 grass top      2 grass side    3 dirt          4 stone         5 cobblestone
      *>   6 oak planks     7 oak log top   8 oak log bark  9 oak leaves   10 glass
      *>  11 bricks
      *> and the pieces of the HUD, drawn in the top-left corner of the picture:
      *>  12 a full heart  13 an empty heart  14 a half heart  (9 x 8)
      *>  15 to 24 the digits 0 to 9, white with a dark drop shadow (4 x 6)
      *> A pixel left "#00000000" is transparent: the holes of the leaves, the pane of the glass.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
      *> The shades of the tile being painted; WS-PICK chooses one of them.
       01 WS-PALETTE-TEXT                        PIC    X(54).
       01 WS-PALETTE REDEFINES WS-PALETTE-TEXT.
           05 WS-SHADE                           PIC    X(9)    OCCURS 6.
       01 WS-PICK                                PIC    9.
       01 WS-CHANCE                              PIC    9V9(6).
       01 WS-X                                   PIC    99.
       01 WS-Y                                   PIC    99.
       01 WS-INDEX                               PIC    999.
       01 WS-ROW                                 PIC    99.
       01 WS-PHASE                               PIC    99.
       01 WS-DISTANCE                            PIC    9(3)V9(4).

      *> The heart, 9 x 8: O outline, R red, W the shine, D the shade low on it.
       01 WS-HEART-DATA.
           05 FILLER                             PIC    X(9)    VALUE "..OO.OO..".
           05 FILLER                             PIC    X(9)    VALUE ".ORRORRO.".
           05 FILLER                             PIC    X(9)    VALUE "ORWRRRRRO".
           05 FILLER                             PIC    X(9)    VALUE "ORRRRRRRO".
           05 FILLER                             PIC    X(9)    VALUE ".ORRRRRO.".
           05 FILLER                             PIC    X(9)    VALUE "..ODRDO..".
           05 FILLER                             PIC    X(9)    VALUE "...ODO...".
           05 FILLER                             PIC    X(9)    VALUE "....O....".
       01 WS-HEART REDEFINES WS-HEART-DATA.
           05 WS-HEART-ROW                       PIC    X(9)    OCCURS 8.
       01 WS-HEART-CELL                          PIC    X.

      *> The digits 0 to 9, 3 x 5 each, row after row.
       01 WS-DIGIT-DATA.
           05 FILLER                             PIC    X(15)   VALUE "XXXX.XX.XX.XXXX".
           05 FILLER                             PIC    X(15)   VALUE ".X.XX..X..X.XXX".
           05 FILLER                             PIC    X(15)   VALUE "XXX..XXXXX..XXX".
           05 FILLER                             PIC    X(15)   VALUE "XXX..X.XX..XXXX".
           05 FILLER                             PIC    X(15)   VALUE "X.XX.XXXX..X..X".
           05 FILLER                             PIC    X(15)   VALUE "XXXX..XXX..XXXX".
           05 FILLER                             PIC    X(15)   VALUE "XXXX..XXXX.XXXX".
           05 FILLER                             PIC    X(15)   VALUE "XXX..X.X..X..X.".
           05 FILLER                             PIC    X(15)   VALUE "XXXX.XXXXX.XXXX".
           05 FILLER                             PIC    X(15)   VALUE "XXXX.XXXX..XXXX".
       01 WS-DIGITS REDEFINES WS-DIGIT-DATA.
           05 WS-DIGIT-PATTERN                   PIC    X(15)   OCCURS 10.
       01 WS-DIGIT                               PIC    99.

       LINKAGE SECTION.
       01 LK-TILE                                PIC    99.
       01 LK-PIXELS.
           05 LK-PIXEL                           PIC    X(9)    OCCURS 256.

       PROCEDURE DIVISION USING LK-TILE LK-PIXELS.
           PERFORM VARYING WS-INDEX FROM 1 BY 1 UNTIL WS-INDEX > 256
               SET     LK-PIXEL(WS-INDEX)            TO    "#00000000"
           END-PERFORM

           *> One seed per tile: the same tile is the same picture on every run.

           COMPUTE WS-CHANCE = FUNCTION RANDOM( LK-TILE * 7919 + 13 )

           EVALUATE TRUE
               WHEN LK-TILE >= 12 AND LK-TILE <= 14
                   PERFORM PAINT-HEART
               WHEN LK-TILE >= 15 AND LK-TILE <= 24
                   PERFORM PAINT-DIGIT
               WHEN OTHER
                   PERFORM VARYING WS-Y FROM 0 BY 1 UNTIL WS-Y > 15
                       PERFORM VARYING WS-X FROM 0 BY 1 UNTIL WS-X > 15
                           COMPUTE WS-INDEX = WS-Y * 16 + WS-X + 1
                           COMPUTE WS-CHANCE = FUNCTION RANDOM
                           PERFORM PAINT-BLOCK-PIXEL
                       END-PERFORM
                   END-PERFORM
           END-EVALUATE
           GOBACK.

       PAINT-BLOCK-PIXEL.
           EVALUATE LK-TILE
               WHEN 1
                   PERFORM PAINT-GRASS
               WHEN 2

                   *> The side of a grass block: dirt with the turf hanging over its top edge, two to
                   *> five pixels deep, a little deeper or shallower from one column to the next.

                   COMPUTE WS-PHASE = 2 + FUNCTION MOD( WS-X * 7 + 3, 4 )
                   IF    WS-Y < WS-PHASE OR ( WS-Y = WS-PHASE AND WS-CHANCE < 0.4 )
                       PERFORM PAINT-GRASS
                   ELSE
                       PERFORM PAINT-DIRT
                   END-IF
               WHEN 3
                   PERFORM PAINT-DIRT
               WHEN 4
                   PERFORM PAINT-STONE
               WHEN 5
                   PERFORM PAINT-COBBLESTONE
               WHEN 6
                   PERFORM PAINT-PLANKS
               WHEN 7
                   PERFORM PAINT-LOG-TOP
               WHEN 8
                   PERFORM PAINT-BARK
               WHEN 9
                   PERFORM PAINT-LEAVES
               WHEN 10
                   PERFORM PAINT-GLASS
               WHEN 11
                   PERFORM PAINT-BRICKS
           END-EVALUATE.

       PAINT-GRASS.
           SET     WS-PALETTE-TEXT               TO    "#5B9E38FF#66AC40FF#54932FFF#72BA4AFF#4C8A2CFF#7FC456FF"
           PERFORM PICK-WEIGHTED
           SET     LK-PIXEL(WS-INDEX)            TO    WS-SHADE(WS-PICK).

       PAINT-DIRT.
           SET     WS-PALETTE-TEXT               TO    "#866043FF#7A563CFF#93694AFF#6E4C33FF#9C7656FF#5E4029FF"
           PERFORM PICK-WEIGHTED
           SET     LK-PIXEL(WS-INDEX)            TO    WS-SHADE(WS-PICK).

       PAINT-STONE.

           *> Stone: grey grains, a third of them carried on from the pixel to the left, which
           *> draws the short horizontal streaks of a rock face.

           IF    WS-X > 0 AND WS-CHANCE < 0.35
               SET     LK-PIXEL(WS-INDEX)            TO    LK-PIXEL(WS-INDEX - 1)
           ELSE
               COMPUTE WS-CHANCE = FUNCTION RANDOM
               SET     WS-PALETTE-TEXT               TO    "#7D7D7DFF#878787FF#737373FF#6A6A6AFF#929292FF#5F5F5FFF"
               PERFORM PICK-WEIGHTED
               SET     LK-PIXEL(WS-INDEX)            TO    WS-SHADE(WS-PICK)
           END-IF.

       PAINT-COBBLESTONE.

           *> Cobblestone: courses of stones four pixels high and five wide, each course shifted by
           *> two; dark mortar between them, a lighter top-left edge on every stone.

           COMPUTE WS-ROW = WS-Y / 4
           COMPUTE WS-PHASE = FUNCTION MOD( WS-X + WS-ROW * 2, 5 )
           IF    WS-PHASE = 0 OR FUNCTION MOD( WS-Y, 4 ) = 0
               IF    WS-CHANCE < 0.5
                   SET     LK-PIXEL(WS-INDEX)            TO    "#545454FF"
               ELSE
                   SET     LK-PIXEL(WS-INDEX)            TO    "#4A4A4AFF"
               END-IF
           ELSE
               SET     WS-PALETTE-TEXT               TO    "#8E8E8EFF#7B7B7BFF#9C9C9CFF#6F6F6FFF#A8A8A8FF#666666FF"
               COMPUTE WS-PICK = FUNCTION MOD( FUNCTION INTEGER( ( WS-X + WS-ROW * 2 ) / 5 ) * 37 + WS-ROW * 11, 4 ) + 1
               IF    WS-PHASE = 1 OR FUNCTION MOD( WS-Y, 4 ) = 1
                   SET     WS-PICK               TO    5
               END-IF
               IF    WS-CHANCE < 0.12
                   SET     WS-PICK               TO    6
               END-IF
               SET     LK-PIXEL(WS-INDEX)            TO    WS-SHADE(WS-PICK)
           END-IF.

       PAINT-PLANKS.

           *> Oak planks: four boards, a dark seam under each, one butt joint per board, and a few
           *> darker grains.

           COMPUTE WS-ROW = WS-Y / 4
           EVALUATE TRUE
               WHEN FUNCTION MOD( WS-Y, 4 ) = 3
                   SET     LK-PIXEL(WS-INDEX)            TO    "#5E4626FF"
               WHEN WS-X = FUNCTION MOD( WS-ROW * 7 + 5, 16 )
                   SET     LK-PIXEL(WS-INDEX)            TO    "#6B522EFF"
               WHEN WS-CHANCE < 0.12
                   SET     LK-PIXEL(WS-INDEX)            TO    "#8C6C3DFF"
               WHEN OTHER
                   SET     WS-PALETTE-TEXT               TO    "#A3834FFF#9B7B48FF#AE8D58FF#957443FF#A3834FFF#9B7B48FF"
                   COMPUTE WS-PICK = FUNCTION MOD( WS-ROW, 4 ) + 1
                   SET     LK-PIXEL(WS-INDEX)            TO    WS-SHADE(WS-PICK)
           END-EVALUATE.

       PAINT-LOG-TOP.

           *> The cut end of a log: growth rings round the middle, bark round the edge.

           COMPUTE WS-DISTANCE = FUNCTION SQRT( ( WS-X - 7.5 ) * ( WS-X - 7.5 ) + ( WS-Y - 7.5 ) * ( WS-Y - 7.5 ) )
           EVALUATE TRUE
               WHEN WS-DISTANCE > 6.6
                   IF    WS-CHANCE < 0.5
                       SET     LK-PIXEL(WS-INDEX)            TO    "#5D4428FF"
                   ELSE
                       SET     LK-PIXEL(WS-INDEX)            TO    "#6B5032FF"
                   END-IF
               WHEN WS-DISTANCE < 1.2
                   SET     LK-PIXEL(WS-INDEX)            TO    "#8E6B3EFF"
               WHEN FUNCTION MOD( FUNCTION INTEGER( WS-DISTANCE * 1.1 ), 2 ) = 0
                   SET     LK-PIXEL(WS-INDEX)            TO    "#B89462FF"
               WHEN OTHER
                   SET     LK-PIXEL(WS-INDEX)            TO    "#A37F4EFF"
           END-EVALUATE.

       PAINT-BARK.

           *> Bark: vertical ridges, each column its own shade, a deep groove every fourth column.

           SET     WS-PALETTE-TEXT               TO    "#6B5032FF#5D4428FF#77593AFF#4F3922FF#45311DFF#83653FFF"
           COMPUTE WS-PICK = FUNCTION MOD( WS-X * 13 + 7, 4 ) + 1
           IF    FUNCTION MOD( WS-X + FUNCTION INTEGER( WS-Y / 6 ), 4 ) = 0 OR WS-CHANCE < 0.15
               SET     WS-PICK                   TO    5
           END-IF
           IF    WS-CHANCE > 0.93
               SET     WS-PICK                   TO    6
           END-IF
           SET     LK-PIXEL(WS-INDEX)            TO    WS-SHADE(WS-PICK).

       PAINT-LEAVES.

           *> Leaves: deep greens with one pixel in six left open, so the sky shows through.

           IF    WS-CHANCE < 0.17
               SET     LK-PIXEL(WS-INDEX)            TO    "#00000000"
           ELSE
               COMPUTE WS-CHANCE = FUNCTION RANDOM
               SET     WS-PALETTE-TEXT               TO    "#3E7A25FF#346B1FFF#48892DFF#2D5D1AFF#55983AFF#24501400"
               PERFORM PICK-WEIGHTED
               IF    WS-PICK = 6
                   SET     WS-PICK               TO    4
               END-IF
               SET     LK-PIXEL(WS-INDEX)            TO    WS-SHADE(WS-PICK)
           END-IF.

       PAINT-GLASS.

           *> Glass: a pale frame, two short glints across the pane, and the rest left clear.

           EVALUATE TRUE
               WHEN WS-X = 0 OR WS-X = 15 OR WS-Y = 0 OR WS-Y = 15
                   SET     LK-PIXEL(WS-INDEX)            TO    "#D5E6EDFF"
               WHEN WS-X - WS-Y = 3 AND WS-X > 4 AND WS-X < 9
                   SET     LK-PIXEL(WS-INDEX)            TO    "#F4FBFFFF"
               WHEN WS-X - WS-Y = 5 AND WS-X > 8 AND WS-X < 12
                   SET     LK-PIXEL(WS-INDEX)            TO    "#F4FBFFFF"
               WHEN OTHER
                   SET     LK-PIXEL(WS-INDEX)            TO    "#00000000"
           END-EVALUATE.

       PAINT-BRICKS.

           *> Bricks: courses four pixels high, bricks eight long, every other course shifted by
           *> half a brick, pale mortar between them.

           COMPUTE WS-ROW = WS-Y / 4
           COMPUTE WS-PHASE = FUNCTION MOD( WS-X + FUNCTION MOD( WS-ROW, 2 ) * 4, 8 )
           IF    FUNCTION MOD( WS-Y, 4 ) = 3 OR WS-PHASE = 7
               IF    WS-CHANCE < 0.5
                   SET     LK-PIXEL(WS-INDEX)            TO    "#B9B1A6FF"
               ELSE
                   SET     LK-PIXEL(WS-INDEX)            TO    "#A69E93FF"
               END-IF
           ELSE
               SET     WS-PALETTE-TEXT               TO    "#9A513EFF#8C4635FF#A65C46FF#81402FFF#B06A52FF#9A513EFF"
               COMPUTE WS-PICK = FUNCTION MOD( FUNCTION INTEGER( ( WS-X + FUNCTION MOD( WS-ROW, 2 ) * 4 ) / 8 ) * 3 + WS-ROW * 5, 4 ) + 1
               IF    WS-CHANCE < 0.1
                   SET     WS-PICK               TO    5
               END-IF
               SET     LK-PIXEL(WS-INDEX)            TO    WS-SHADE(WS-PICK)
           END-IF.

       PAINT-HEART.

           *> 12 full, 13 empty (a dark socket), 14 half: red on the left half, dark on the right.

           PERFORM VARYING WS-Y FROM 0 BY 1 UNTIL WS-Y > 7
               PERFORM VARYING WS-X FROM 0 BY 1 UNTIL WS-X > 8
                   COMPUTE WS-INDEX = WS-Y * 16 + WS-X + 1
                   MOVE    WS-HEART-ROW(WS-Y + 1)(WS-X + 1:1)    TO    WS-HEART-CELL
                   IF    WS-HEART-CELL NOT = "." AND WS-HEART-CELL NOT = "O"
                       IF    LK-TILE = 13 OR ( LK-TILE = 14 AND WS-X > 4 )
                           SET     WS-HEART-CELL TO    "S"
                       END-IF
                   END-IF
                   EVALUATE WS-HEART-CELL
                       WHEN "O"
                           SET     LK-PIXEL(WS-INDEX)            TO    "#1A0A0AFF"
                       WHEN "R"
                           SET     LK-PIXEL(WS-INDEX)            TO    "#D8262EFF"
                       WHEN "W"
                           SET     LK-PIXEL(WS-INDEX)            TO    "#FF9A9AFF"
                       WHEN "D"
                           SET     LK-PIXEL(WS-INDEX)            TO    "#A3141BFF"
                       WHEN "S"
                           SET     LK-PIXEL(WS-INDEX)            TO    "#3A2222FF"
                   END-EVALUATE
               END-PERFORM
           END-PERFORM.

       PAINT-DIGIT.

           *> A digit of the pixel font: the white glyph over its shadow, one pixel down and right.

           COMPUTE WS-DIGIT = LK-TILE - 14
           PERFORM VARYING WS-Y FROM 0 BY 1 UNTIL WS-Y > 4
               PERFORM VARYING WS-X FROM 0 BY 1 UNTIL WS-X > 2
                   IF    WS-DIGIT-PATTERN(WS-DIGIT)(WS-Y * 3 + WS-X + 1:1) = "X"
                       COMPUTE WS-INDEX = ( WS-Y + 1 ) * 16 + WS-X + 2
                       SET     LK-PIXEL(WS-INDEX)            TO    "#3F3F3FFF"
                   END-IF
               END-PERFORM
           END-PERFORM
           PERFORM VARYING WS-Y FROM 0 BY 1 UNTIL WS-Y > 4
               PERFORM VARYING WS-X FROM 0 BY 1 UNTIL WS-X > 2
                   IF    WS-DIGIT-PATTERN(WS-DIGIT)(WS-Y * 3 + WS-X + 1:1) = "X"
                       COMPUTE WS-INDEX = WS-Y * 16 + WS-X + 1
                       SET     LK-PIXEL(WS-INDEX)            TO    "#FFFFFFFF"
                   END-IF
               END-PERFORM
           END-PERFORM.

       PICK-WEIGHTED.

           *> Chooses one of six shades from WS-CHANCE, the first ones more often than the last.

           EVALUATE TRUE
               WHEN WS-CHANCE < 0.30
                   SET     WS-PICK               TO    1
               WHEN WS-CHANCE < 0.55
                   SET     WS-PICK               TO    2
               WHEN WS-CHANCE < 0.72
                   SET     WS-PICK               TO    3
               WHEN WS-CHANCE < 0.84
                   SET     WS-PICK               TO    4
               WHEN WS-CHANCE < 0.94
                   SET     WS-PICK               TO    5
               WHEN OTHER
                   SET     WS-PICK               TO    6
           END-EVALUATE.
