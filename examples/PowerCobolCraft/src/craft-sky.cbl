       IDENTIFICATION DIVISION.
       PROGRAM-ID. CRAFT-SKY.

      *> The sky at a time of day. CobolCraft keeps the world's time in ticks, 24000 to a day:
      *> 0 sunrise, 1000 "day", 6000 "noon", 12000 sunset, 13000 "night", 18000 "midnight"
      *> (the words /time set takes). From the time this works out the two colours of the sky
      *> gradient, the fog that matches the horizon, where the sun stands, how strong and what
      *> colour its light is, the ambient light, and how bright the stars are.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-HEX-DIGITS                          PIC    X(16)   VALUE "0123456789ABCDEF".
       01 WS-ANGLE                               PIC    S9V9(8).
       01 WS-ELEVATION                           PIC    S9V9(8).
       01 WS-DAYLIGHT                            PIC    S9V9(8).
       01 WS-TWILIGHT                            PIC    S9V9(8).

      *> One colour being mixed, 0 to 255 a channel, and its "#RRGGBBFF" spelling.
       01 WS-RED                                 PIC    S9(3)V9(4).
       01 WS-GREEN                               PIC    S9(3)V9(4).
       01 WS-BLUE                                PIC    S9(3)V9(4).
       01 WS-CHANNEL                             PIC    999.
       01 WS-COLOR                               PIC    X(9).
       01 WS-POINTER                             PIC    99.

       LINKAGE SECTION.
       01 LK-TIME                                PIC    9(5).
       01 LK-SKY.
           05 LK-SKY-TOP                         PIC    X(9).
           05 LK-SKY-BOTTOM                      PIC    X(9).
           05 LK-FOG-RED                         PIC    9V9(4).
           05 LK-FOG-GREEN                       PIC    9V9(4).
           05 LK-FOG-BLUE                        PIC    9V9(4).
           05 LK-SUN-ANGLE                       PIC    S9(3)V9(4).
           05 LK-DAYLIGHT                        PIC    9V9(4).
           05 LK-LIGHT-INTENSITY                 PIC    9V9(4).
           05 LK-LIGHT-RED                       PIC    9V9(4).
           05 LK-LIGHT-GREEN                     PIC    9V9(4).
           05 LK-LIGHT-BLUE                      PIC    9V9(4).
           05 LK-AMBIENT                         PIC    9V9(4).
           05 LK-STARS                           PIC    9V9(4).

       PROCEDURE DIVISION USING LK-TIME LK-SKY.

           *> The sun goes round once a day: up in the east at tick 0, overhead at 6000.

           COMPUTE WS-ANGLE = FUNCTION MOD( LK-TIME, 24000 ) / 24000 * 6.28318531
           COMPUTE WS-ELEVATION = FUNCTION SIN( WS-ANGLE )
           COMPUTE LK-SUN-ANGLE = FUNCTION MOD( LK-TIME, 24000 ) / 24000 * 360

           *> Daylight fades in over the sunrise and out over the sunset; twilight is strongest with
           *> the sun on the horizon and gone once it is a quarter of the way up or down.

           COMPUTE WS-DAYLIGHT = ( WS-ELEVATION + 0.12 ) / 0.42
           IF    WS-DAYLIGHT < 0
               SET     WS-DAYLIGHT                   TO    0
           END-IF
           IF    WS-DAYLIGHT > 1
               SET     WS-DAYLIGHT                   TO    1
           END-IF
           COMPUTE WS-TWILIGHT = 1 - FUNCTION ABS( WS-ELEVATION ) / 0.28
           IF    WS-TWILIGHT < 0
               SET     WS-TWILIGHT                   TO    0
           END-IF
           SET     LK-DAYLIGHT                   TO    WS-DAYLIGHT

           *> The top of the sky: deep night blue to a clear day blue, a touch of violet at dusk.

           COMPUTE WS-RED = 4 + ( 92 - 4 ) * WS-DAYLIGHT
           COMPUTE WS-GREEN = 6 + ( 143 - 6 ) * WS-DAYLIGHT
           COMPUTE WS-BLUE = 16 + ( 230 - 16 ) * WS-DAYLIGHT
           COMPUTE WS-RED = WS-RED + ( 96 - WS-RED ) * WS-TWILIGHT * 0.25
           COMPUTE WS-GREEN = WS-GREEN + ( 74 - WS-GREEN ) * WS-TWILIGHT * 0.25
           COMPUTE WS-BLUE = WS-BLUE + ( 140 - WS-BLUE ) * WS-TWILIGHT * 0.25
           PERFORM SPELL-COLOR
           SET     LK-SKY-TOP                    TO    WS-COLOR

           *> The horizon: pale blue by day, ink by night, and the orange glow of a sunset.

           COMPUTE WS-RED = 12 + ( 176 - 12 ) * WS-DAYLIGHT
           COMPUTE WS-GREEN = 20 + ( 206 - 20 ) * WS-DAYLIGHT
           COMPUTE WS-BLUE = 40 + ( 255 - 40 ) * WS-DAYLIGHT
           COMPUTE WS-RED = WS-RED + ( 242 - WS-RED ) * WS-TWILIGHT * 0.7
           COMPUTE WS-GREEN = WS-GREEN + ( 146 - WS-GREEN ) * WS-TWILIGHT * 0.7
           COMPUTE WS-BLUE = WS-BLUE + ( 82 - WS-BLUE ) * WS-TWILIGHT * 0.7
           PERFORM SPELL-COLOR
           SET     LK-SKY-BOTTOM                 TO    WS-COLOR
           COMPUTE LK-FOG-RED = WS-RED / 255
           COMPUTE LK-FOG-GREEN = WS-GREEN / 255
           COMPUTE LK-FOG-BLUE = WS-BLUE / 255

           *> The light: the warm sun by day, reddening on the horizon; the cold moon by night.

           IF    WS-ELEVATION > -0.06
               COMPUTE LK-LIGHT-INTENSITY = 0.08 + 2.1 * WS-DAYLIGHT
               COMPUTE LK-LIGHT-RED = 1
               COMPUTE LK-LIGHT-GREEN = 0.96 - 0.36 * WS-TWILIGHT
               COMPUTE LK-LIGHT-BLUE = 0.88 - 0.53 * WS-TWILIGHT
           ELSE
               COMPUTE LK-LIGHT-INTENSITY = 0.32
               COMPUTE LK-LIGHT-RED = 0.62
               COMPUTE LK-LIGHT-GREEN = 0.72
               COMPUTE LK-LIGHT-BLUE = 1
           END-IF
           COMPUTE LK-AMBIENT = 0.16 + 0.44 * WS-DAYLIGHT
           COMPUTE LK-STARS = 0 - WS-ELEVATION * 3
           IF    WS-ELEVATION > 0
               SET     LK-STARS                      TO    0
           END-IF
           IF    LK-STARS > 1
               SET     LK-STARS                      TO    1
           END-IF
           GOBACK.

       SPELL-COLOR.

           *> WS-RED, WS-GREEN and WS-BLUE written as "#RRGGBBFF", two hexadecimal digits a channel.

           SET     WS-COLOR                      TO    "#000000FF"
           SET     WS-POINTER                    TO    2
           COMPUTE WS-CHANNEL = WS-RED
           PERFORM SPELL-CHANNEL
           COMPUTE WS-CHANNEL = WS-GREEN
           PERFORM SPELL-CHANNEL
           COMPUTE WS-CHANNEL = WS-BLUE
           PERFORM SPELL-CHANNEL.

       SPELL-CHANNEL.
           IF    WS-CHANNEL > 255
               SET     WS-CHANNEL                    TO    255
           END-IF
           MOVE    WS-HEX-DIGITS(FUNCTION INTEGER( WS-CHANNEL / 16 ) + 1:1)   TO    WS-COLOR(WS-POINTER:1)
           MOVE    WS-HEX-DIGITS(FUNCTION MOD( WS-CHANNEL, 16 ) + 1:1)  TO    WS-COLOR(WS-POINTER + 1:1)
           ADD     2                             TO    WS-POINTER.
