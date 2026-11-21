# PistonPort artwork

Graphite panels, a silver selection mark and an off-white Play button keep attention on the local cover art and the gold trophy. PlayStation face buttons keep their identification colors.

The built-in ImageGen used the supplied trophy thumbnail as its reference. PistonCup.png is the transparent master. make_icon.py exports the PNG and Windows ICO for the launcher. BuildRuntime.ps1 creates the game executable's Windows icon from the local dump's `sce_sys/icon0.png`.

The welcome screen validates a PS4 PKG or extracted game folder before opening Garage. The Garage then displays `sce_sys/pic1.png`, which includes the original game logo. When the game source has no cover art, the Garage shows the game title.

## ImageGen prompt

Create a square Windows racing game app icon, a polished gold winged Piston Cup trophy inspired by the provided reference: a front-facing symmetric engine piston crown with two clearly machined horizontal grooves, three short stepped golden wings on each side, a thick connecting rod and round pin hole, and a charcoal two-tier trophy plinth. Strong simple silhouette, refined enamel and restrained metallic highlights, readable at 32 pixels. Fill about 90 percent of the square. Isolated with a truly transparent background. No text, initials, tile, environmental shadows, scenery or extra symbols.
