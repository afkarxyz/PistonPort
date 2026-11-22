# Embed the same DualShock PNGs used by the Rune launcher's Controls tab.
set(pistonport_pad_assets "${CMAKE_CURRENT_SOURCE_DIR}/../launcher-rust/assets/pads")
set(pistonport_icon_header "#pragma once\nnamespace PistonPortPadIcons {\n")
foreach(icon IN ITEMS
    icons/playstation/playstation_stick_l.png
    icons/playstation/playstation_trigger_r2.png
    icons/playstation/playstation_trigger_l2.png
    icons/playstation/playstation_button_color_square.png
    icons/playstation/playstation_button_color_cross.png
    icons/playstation/playstation_button_color_circle.png
    icons/playstation/playstation4_button_options.png
    icons/playstation/playstation_button_color_triangle.png
    icons/playstation/playstation_trigger_l1.png
    icons/playstation/playstation_trigger_r1.png
    icons/playstation/playstation_button_l3.png
    icons/playstation/playstation_button_r3.png
    icons/playstation/playstation_dpad_up.png
    icons/playstation/playstation_dpad_down.png
    icons/playstation/playstation_dpad_left.png
    icons/playstation/playstation_dpad_right.png)
    set(path "${pistonport_pad_assets}/${icon}")
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${path}")
    get_filename_component(name "${icon}" NAME_WE)
    file(READ "${path}" bytes HEX)
    string(REGEX REPLACE "(..)" "0x\\1," bytes "${bytes}")
    string(APPEND pistonport_icon_header "inline constexpr unsigned char ${name}[] = {${bytes}};\n")
endforeach()
string(APPEND pistonport_icon_header "}\n")
file(CONFIGURE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/pistonport_hint_icons.h"
    CONTENT "${pistonport_icon_header}" @ONLY)
