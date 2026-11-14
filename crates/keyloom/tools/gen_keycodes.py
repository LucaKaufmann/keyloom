# Generates data/keycodes.json (spec 07). Offline: the table below is the source; fixture codes mark "verified".
# Usage: python3 -I crates/keyloom/tools/gen_keycodes.py <repo root> crates/keyloom/data/keycodes.json
import json, re, sys, glob
repo = sys.argv[1]
out = sys.argv[2]
seen = set()
for f in glob.glob(repo + "/crates/keyloom/tests/fixtures/*.json"):
    seen |= set(re.findall(r'"code":\s*"([^"]+)"', open(f).read()))

E = []
def k(code, cat, desc, legend, aliases=(), **extra):
    e = {"code": code, "aliases": list(aliases), "category": cat, "description": desc, "legend": legend, "oryx": True}
    e["verified"] = code in seen
    e.update(extra)
    E.append(e)

# ---- basic: letters, digits
for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ":
    k(f"KC_{c}", "basic", f"Letter {c}", c, [c])
for d in "1234567890":
    k(f"KC_{d}", "basic", f"Digit {d} (number row; shifted gives the US symbol above it)", d, [d])
# punctuation (unshifted US)
P = [
 ("KC_MINUS", ["KC_MINS", "-", "Minus", "Dash", "Hyphen"], "Minus / hyphen (shift: _)", "-"),
 ("KC_EQUAL", ["KC_EQL", "=", "Equal", "Equals"], "Equal sign (shift: +)", "="),
 ("KC_LBRC", ["KC_LEFT_BRACKET", "[", "Left Bracket", "LBracket"], "Left square bracket (shift: {)", "["),
 ("KC_RBRC", ["KC_RIGHT_BRACKET", "]", "Right Bracket", "RBracket"], "Right square bracket (shift: })", "]"),
 ("KC_BSLS", ["KC_BACKSLASH", "\\", "Backslash"], "Backslash (shift: |)", "\\"),
 ("KC_SCLN", ["KC_SEMICOLON", ";", "Semicolon"], "Semicolon (shift: :)", ";"),
 ("KC_QUOTE", ["KC_QUOT", "'", "Quote", "Apostrophe", "Single Quote"], "Apostrophe / single quote (shift: \")", "'"),
 ("KC_GRAVE", ["KC_GRV", "`", "Grave", "Backtick", "Backquote"], "Grave accent / backtick (shift: ~)", "`"),
 ("KC_COMMA", ["KC_COMM", ",", "Comma"], "Comma (shift: <)", ","),
 ("KC_DOT", [".", "Dot", "Period", "Full Stop"], "Period / dot (shift: >)", "."),
 ("KC_SLASH", ["KC_SLSH", "/", "Slash", "Forward Slash"], "Forward slash (shift: ?)", "/"),
 ("KC_NONUS_HASH", ["KC_NUHS", "ISO Hash", "Non-US Hash"], "ISO key left of Enter (# and ~ on UK layouts). Rarely needed on US layouts", "ISO#"),
 ("KC_NONUS_BACKSLASH", ["KC_NUBS", "ISO Backslash", "Non-US Backslash"], "ISO key right of left Shift (< > on German, \\ | on UK layouts)", "ISO\\"),
]
for code, al, desc, leg in P:
    k(code, "basic", desc, leg, al)
# shifted symbols (US): Oryx uses the QMK short names
S = [
 ("KC_TILD", ["KC_TILDE", "~", "Tilde"], "Tilde (Shift + `)", "~"),
 ("KC_EXLM", ["KC_EXCLAIM", "!", "Exclamation", "Exclamation Mark", "Bang"], "Exclamation mark (Shift + 1)", "!"),
 ("KC_AT", ["@", "At", "At Sign"], "At sign (Shift + 2)", "@"),
 ("KC_HASH", ["#", "Hash", "Number Sign", "Octothorpe"], "Hash (Shift + 3)", "#"),
 ("KC_DLR", ["KC_DOLLAR", "$", "Dollar"], "Dollar (Shift + 4)", "$"),
 ("KC_PERC", ["KC_PERCENT", "%", "Percent"], "Percent (Shift + 5)", "%"),
 ("KC_CIRC", ["KC_CIRCUMFLEX", "^", "Caret", "Circumflex"], "Caret (Shift + 6)", "^"),
 ("KC_AMPR", ["KC_AMPERSAND", "&", "Ampersand"], "Ampersand (Shift + 7)", "&"),
 ("KC_ASTR", ["KC_ASTERISK", "*", "Asterisk", "Star"], "Asterisk (Shift + 8)", "*"),
 ("KC_LPRN", ["KC_LEFT_PAREN", "(", "Left Paren", "LParen", "Open Paren"], "Left parenthesis (Shift + 9)", "("),
 ("KC_RPRN", ["KC_RIGHT_PAREN", ")", "Right Paren", "RParen", "Close Paren"], "Right parenthesis (Shift + 0)", ")"),
 ("KC_UNDS", ["KC_UNDERSCORE", "_", "Underscore"], "Underscore (Shift + -)", "_"),
 ("KC_PLUS", ["+", "Plus"], "Plus (Shift + =)", "+"),
 ("KC_LCBR", ["KC_LEFT_CURLY_BRACE", "{", "Left Brace", "LBrace", "Left Curly"], "Left curly brace (Shift + [)", "{"),
 ("KC_RCBR", ["KC_RIGHT_CURLY_BRACE", "}", "Right Brace", "RBrace", "Right Curly"], "Right curly brace (Shift + ])", "}"),
 ("KC_PIPE", ["|", "Pipe", "Vertical Bar"], "Pipe (Shift + \\)", "|"),
 ("KC_COLN", ["KC_COLON", ":", "Colon"], "Colon (Shift + ;)", ":"),
 ("KC_DQUO", ["KC_DOUBLE_QUOTE", "KC_DQT", "\"", "Double Quote"], "Double quote (Shift + ')", "\""),
 ("KC_LABK", ["KC_LEFT_ANGLE_BRACKET", "KC_LT", "<", "Less Than", "Left Angle"], "Less-than (Shift + ,)", "<"),
 ("KC_RABK", ["KC_RIGHT_ANGLE_BRACKET", "KC_GT", ">", "Greater Than", "Right Angle"], "Greater-than (Shift + .)", ">"),
 ("KC_QUES", ["KC_QUESTION", "?", "Question", "Question Mark"], "Question mark (Shift + /)", "?"),
]
for code, al, desc, leg in S:
    k(code, "basic", desc + ". Sends Shift + the base key, so it can't be combined with other modifiers reliably", leg, al)

# ---- editing / whitespace
ED = [
 ("KC_ESCAPE", ["KC_ESC", "Esc", "Escape"], "Escape", "Esc"),
 ("KC_ENTER", ["KC_ENT", "Enter", "Return", "Ret", "⏎", "↵"], "Enter / Return", "⏎"),
 ("KC_TAB", ["Tab", "⇥"], "Tab", "⇥"),
 ("KC_SPACE", ["KC_SPC", "Space", "Spc", "Spacebar", "␣"], "Space bar", "␣"),
 ("KC_BSPC", ["KC_BACKSPACE", "Backspace", "Bksp", "BS", "⌫"], "Backspace (delete left)", "⌫"),
 ("KC_DELETE", ["KC_DEL", "Delete", "Del", "Forward Delete", "⌦"], "Forward delete", "⌦"),
 ("KC_INSERT", ["KC_INS", "Insert", "Ins"], "Insert", "Ins"),
 ("KC_CAPS", ["KC_CAPS_LOCK", "Caps", "Caps Lock", "CapsLock", "⇪"], "Caps Lock (consider CW_TOGG Caps Word instead)", "⇪"),
 ("KC_PSCR", ["KC_PRINT_SCREEN", "Print Screen", "PrintScreen", "PrtSc", "PrtScr"], "Print Screen (macOS ignores it; use Cmd+Shift+3/4/5)", "PrtSc"),
 ("KC_SCRL", ["KC_SCROLL_LOCK", "Scroll Lock", "ScrLk"], "Scroll Lock (on macOS: screen brightness down on some setups)", "ScrLk"),
 ("KC_PAUSE", ["KC_PAUS", "KC_BRK", "Pause", "Break"], "Pause / Break", "Pause"),
 ("KC_APPLICATION", ["KC_APP", "Menu", "App", "Context Menu"], "Application / context menu key (Windows, Linux)", "Menu"),
]
for code, al, desc, leg in ED:
    k(code, "editing", desc, leg, al)

# ---- navigation
NAV = [
 ("KC_LEFT", ["Left", "Left Arrow", "←"], "Left arrow", "←"),
 ("KC_RIGHT", ["KC_RGHT", "Right", "Right Arrow", "→"], "Right arrow", "→"),
 ("KC_UP", ["Up", "Up Arrow", "↑"], "Up arrow", "↑"),
 ("KC_DOWN", ["Down", "Down Arrow", "↓"], "Down arrow", "↓"),
 ("KC_HOME", ["Home"], "Home (macOS apps mostly want Cmd+Left instead)", "Home"),
 ("KC_END", ["End"], "End (macOS apps mostly want Cmd+Right instead)", "End"),
 ("KC_PAGE_UP", ["KC_PGUP", "Page Up", "PageUp", "PgUp"], "Page Up", "PgUp"),
 ("KC_PGDN", ["KC_PAGE_DOWN", "Page Down", "PageDown", "PgDn"], "Page Down", "PgDn"),
]
for code, al, desc, leg in NAV:
    k(code, "navigation", desc, leg, al)

# ---- modifiers
MODS = [
 ("KC_LEFT_CTRL", ["KC_LCTL", "KC_LCTRL", "LCTL", "Ctrl", "Control", "LCtrl", "Left Ctrl", "⌃"], "Left Control. Also usable as a hold action (mod-tap: tap = key, hold = this)", "Ctrl"),
 ("KC_LEFT_SHIFT", ["KC_LSFT", "KC_LSHIFT", "LSFT", "Shift", "LShift", "Left Shift", "⇧"], "Left Shift. Also usable as a hold action", "⇧"),
 ("KC_LEFT_ALT", ["KC_LALT", "KC_LOPT", "LALT", "Alt", "Option", "Opt", "Left Alt", "⌥"], "Left Alt / Option. Also usable as a hold action", "Alt"),
 ("KC_LEFT_GUI", ["KC_LGUI", "KC_LCMD", "KC_LWIN", "LGUI", "GUI", "Cmd", "Command", "Win", "Windows", "Super", "Meta", "Left Cmd", "⌘"], "Left GUI: Cmd on macOS, Win on Windows, Super on Linux. Also usable as a hold action", "Gui"),
 ("KC_RIGHT_CTRL", ["KC_RCTL", "KC_RCTRL", "RCTL", "RCtrl", "Right Ctrl"], "Right Control", "RCtrl"),
 ("KC_RIGHT_SHIFT", ["KC_RSFT", "KC_RSHIFT", "RSFT", "RShift", "Right Shift"], "Right Shift", "R⇧"),
 ("KC_RIGHT_ALT", ["KC_RALT", "KC_ROPT", "KC_ALGR", "RALT", "AltGr", "Right Alt", "Right Option"], "Right Alt / Option. AltGr on international layouts (accents, €)", "AltGr"),
 ("KC_RIGHT_GUI", ["KC_RGUI", "KC_RCMD", "KC_RWIN", "RGUI", "Right Cmd", "Right Win"], "Right GUI (Cmd / Win / Super)", "RGui"),
]
for code, al, desc, leg in MODS:
    k(code, "modifier", desc, leg, al)

# ---- function keys
for n in range(1, 25):
    note = "" if n <= 12 else " (no default meaning; handy for app or game binds that must not collide with typing)"
    k(f"KC_F{n}", "function", f"F{n}{note}", f"F{n}", [f"F{n}"])

# ---- numpad
NP = [(f"KC_KP_{d}", [f"KC_P{d}", f"Numpad {d}", f"KP{d}"], f"Numpad {d}. Games and apps see a different key than KC_{d}; needs Num Lock on for digits on Windows/Linux", f"kp{d}") for d in "1234567890"]
NP += [
 ("KC_KP_DOT", ["KC_PDOT", "Numpad .", "Numpad Dot"], "Numpad decimal point", "kp."),
 ("KC_KP_PLUS", ["KC_PPLS", "Numpad +", "Numpad Plus"], "Numpad plus", "kp+"),
 ("KC_KP_MINUS", ["KC_PMNS", "Numpad -", "Numpad Minus"], "Numpad minus", "kp-"),
 ("KC_KP_ASTERISK", ["KC_PAST", "Numpad *", "Numpad Multiply"], "Numpad multiply", "kp*"),
 ("KC_KP_SLASH", ["KC_PSLS", "Numpad /", "Numpad Divide"], "Numpad divide", "kp/"),
 ("KC_KP_ENTER", ["KC_PENT", "Numpad Enter"], "Numpad Enter", "kp⏎"),
 ("KC_KP_EQUAL", ["KC_PEQL", "Numpad ="], "Numpad equals (mostly macOS)", "kp="),
 ("KC_NUM", ["KC_NUM_LOCK", "KC_NLCK", "Num Lock", "NumLock"], "Num Lock (Windows/Linux numpad digits need it on; macOS calls it Clear)", "NumLk"),
]
for code, al, desc, leg in NP:
    k(code, "numpad", desc, leg, al)

# ---- media
MED = [
 ("KC_AUDIO_VOL_UP", ["KC_VOLU", "Volume Up", "Vol Up", "Vol+"], "Volume up", "Vol+"),
 ("KC_AUDIO_VOL_DOWN", ["KC_VOLD", "Volume Down", "Vol Down", "Vol-"], "Volume down", "Vol−"),
 ("KC_AUDIO_MUTE", ["KC_MUTE", "Mute"], "Mute audio", "Mute"),
 ("KC_MEDIA_PLAY_PAUSE", ["KC_MPLY", "Play", "Pause Media", "Play/Pause", "⏯"], "Play / pause", "⏯"),
 ("KC_MEDIA_NEXT_TRACK", ["KC_MNXT", "Next Track", "Next", "⏭"], "Next track", "⏭"),
 ("KC_MEDIA_PREV_TRACK", ["KC_MPRV", "Previous Track", "Prev Track", "Prev", "⏮"], "Previous track", "⏮"),
 ("KC_MEDIA_STOP", ["KC_MSTP", "Stop", "Media Stop", "⏹"], "Stop playback", "⏹"),
 ("KC_MEDIA_FAST_FORWARD", ["KC_MFFD", "Fast Forward"], "Fast forward", "⏩"),
 ("KC_MEDIA_REWIND", ["KC_MRWD", "Rewind"], "Rewind", "⏪"),
 ("KC_MEDIA_EJECT", ["KC_EJCT", "Eject"], "Eject (macOS)", "⏏"),
 ("KC_BRIGHTNESS_UP", ["KC_BRIU", "Brightness Up", "Bright+"], "Screen brightness up", "☀+"),
 ("KC_BRIGHTNESS_DOWN", ["KC_BRID", "Brightness Down", "Bright-"], "Screen brightness down", "☀−"),
]
for code, al, desc, leg in MED:
    k(code, "media", desc, leg, al)

# ---- system / applications
SYS = [
 ("KC_SYSTEM_SLEEP", ["KC_SLEP", "Sleep"], "Put the computer to sleep", "Sleep"),
 ("KC_SYSTEM_WAKE", ["KC_WAKE", "Wake"], "Wake the computer", "Wake"),
 ("KC_SYSTEM_POWER", ["KC_PWR", "Power"], "System power (usually shows the shutdown dialog)", "Power"),
 ("KC_CALCULATOR", ["KC_CALC", "Calculator", "Calc"], "Open the calculator (Windows, Linux)", "Calc"),
 ("KC_MAIL", ["Mail", "Email"], "Open the mail app (Windows, Linux)", "Mail"),
 ("KC_MY_COMPUTER", ["KC_MYCM", "My Computer", "File Manager"], "Open the file manager (Windows, Linux)", "MyPC"),
 ("KC_WWW_SEARCH", ["KC_WSCH", "Browser Search"], "Browser search", "WSrch"),
 ("KC_WWW_HOME", ["KC_WHOM", "Browser Home"], "Browser home", "WHome"),
 ("KC_WWW_BACK", ["KC_WBAK", "Browser Back"], "Browser back", "WBack"),
 ("KC_WWW_FORWARD", ["KC_WFWD", "Browser Forward"], "Browser forward", "WFwd"),
 ("KC_WWW_REFRESH", ["KC_WREF", "Browser Refresh", "Refresh"], "Browser refresh", "WRef"),
 ("KC_WWW_STOP", ["KC_WSTP", "Browser Stop"], "Browser stop", "WStop"),
 ("KC_WWW_FAVORITES", ["KC_WFAV", "Browser Favorites", "Bookmarks"], "Browser favorites", "WFav"),
]
for code, al, desc, leg in SYS:
    k(code, "system", desc, leg, al)

# ---- mouse (Oryx uses the pre-2024 QMK names; MS_* are the newer QMK names)
MS = [
 ("KC_MS_UP", ["KC_MS_U", "MS_UP", "Mouse Up"], "Move the pointer up", "M↑"),
 ("KC_MS_DOWN", ["KC_MS_D", "MS_DOWN", "Mouse Down"], "Move the pointer down", "M↓"),
 ("KC_MS_LEFT", ["KC_MS_L", "MS_LEFT", "Mouse Left"], "Move the pointer left", "M←"),
 ("KC_MS_RIGHT", ["KC_MS_R", "MS_RGHT", "Mouse Right"], "Move the pointer right", "M→"),
 ("KC_MS_BTN1", ["KC_BTN1", "MS_BTN1", "Click", "Left Click", "LClick", "Mouse 1", "Mouse Button 1"], "Left mouse button", "Click"),
 ("KC_MS_BTN2", ["KC_BTN2", "MS_BTN2", "Right Click", "RClick", "Mouse 2", "Mouse Button 2"], "Right mouse button", "RClick"),
 ("KC_MS_BTN3", ["KC_BTN3", "MS_BTN3", "Middle Click", "MClick", "Mouse 3", "Mouse Button 3"], "Middle mouse button", "MClick"),
 ("KC_MS_BTN4", ["KC_BTN4", "MS_BTN4", "Mouse 4", "Mouse Button 4", "Mouse Back"], "Mouse button 4 (usually Back)", "M4"),
 ("KC_MS_BTN5", ["KC_BTN5", "MS_BTN5", "Mouse 5", "Mouse Button 5", "Mouse Forward"], "Mouse button 5 (usually Forward)", "M5"),
 ("KC_MS_WH_UP", ["KC_WH_U", "MS_WHLU", "Wheel Up", "Scroll Up"], "Scroll wheel up", "W↑"),
 ("KC_MS_WH_DOWN", ["KC_WH_D", "MS_WHLD", "Wheel Down", "Scroll Down"], "Scroll wheel down", "W↓"),
 ("KC_MS_WH_LEFT", ["KC_WH_L", "MS_WHLL", "Wheel Left", "Scroll Left"], "Scroll wheel left", "W←"),
 ("KC_MS_WH_RIGHT", ["KC_WH_R", "MS_WHLR", "Wheel Right", "Scroll Right"], "Scroll wheel right", "W→"),
 ("KC_MS_ACCEL0", ["KC_ACL0", "MS_ACL0", "Mouse Accel 0"], "Mouse-key speed 0 (slow, while held)", "Acc0"),
 ("KC_MS_ACCEL1", ["KC_ACL1", "MS_ACL1", "Mouse Accel 1"], "Mouse-key speed 1 (medium, while held)", "Acc1"),
 ("KC_MS_ACCEL2", ["KC_ACL2", "MS_ACL2", "Mouse Accel 2"], "Mouse-key speed 2 (fast, while held)", "Acc2"),
]
for code, al, desc, leg in MS:
    k(code, "mouse", desc, leg, al)

# ---- layer actions (Oryx: code + layer field)
LY = [
 ("MO", ["Momentary"], "Momentary layer: active while held. LT (tap a key, hold a layer) is not a separate Oryx code: it is tap = key + hold = MO(n), written LT(n, KC_x) in keyloom", "MO"),
 ("TG", ["Toggle Layer"], "Toggle layer: tap to turn on, tap again to turn off. Put a TG to the same layer on the layer itself so the user can leave", "⇄"),
 ("TO", ["Switch Layer", "Go To Layer"], "Switch to layer: turns that layer on and every other layer (except the default) off. TO(0) is the universal way home", "→"),
 ("TT", ["Tap Toggle"], "Tap-toggle: hold for momentary, tap (QMK default: 5 taps, Oryx may differ) to toggle on. Uses tap-hold timing", "TT"),
 ("OSL", ["One Shot Layer"], "One-shot layer: the next key press comes from that layer, then it turns off", "1×"),
 ("DF", ["Default Layer", "Set Default Layer"], "Set default (base) layer until power-off. Not yet seen in a real Oryx layout: check Oryx offers it before relying on it", "⌂"),
]
for code, al, desc, leg in LY:
    k(code, "layer", desc, leg, al, layer=True, verified=code in seen)

# ---- whole-key forms keyloom writes (not Oryx codes): Oryx stores them as tap + hold
k("LT", "layer", "Layer-tap, written LT(layer, KC_x): tap sends the key, hold switches to the layer. Not an Oryx code: stored as tap = key + hold = MO(layer). (The '<' key is KC_LT.)", "LT", ["Layer Tap", "Layer-Tap"], family=True)
k("MT", "mod-tap", "Mod-tap, written MT(LCTL, KC_x) or LCTL_T(KC_x): tap sends the key, hold is the modifier. Not an Oryx code: stored as tap = key + hold = modifier. Oryx mod-taps take one modifier", "MT", ["Mod Tap", "Mod-Tap"], family=True)
k("TAP_HOLD", "mod-tap", "Tap one key, hold another, written TAP_HOLD(KC_x, KC_y); `_` leaves a side empty. Oryx compiles it to a custom dual-function key", "T/H", ["Tap Hold", "Dual Function"], family=True)

# ---- mod-tap hold codes
MT = [
 ("MEH_T", ["MEH", "KC_MEH"], "Meh (Ctrl+Alt+Shift) as a hold action: tap = key, hold = Meh. Oryx compiles it to MEH_T(KC_x)", "Meh"),
 ("ALL_T", ["HYPR_T", "HYPR", "Hyper", "KC_HYPR"], "Hyper (Ctrl+Alt+Shift+GUI) as a hold action: tap = key, hold = Hyper. Assumed to mirror MEH_T; not yet seen in a real Oryx layout", "Hyper"),
]
for code, al, desc, leg in MT:
    k(code, "mod-tap", desc, leg, al, modTap=True)

# ---- one-shot mods
k("OSM", "one-shot", "One-shot modifier family: the modifier applies to the next key only. Not supported by keyloom yet (OSM(...) is rejected): Oryx offers one-shot modifiers but how it stores them isn't confirmed, so they are set by hand in Oryx", "1×Mod", ["One Shot Mod", "One Shot Modifier"], oneShot=True)

# ---- RGB lighting (Voyager uses RGB matrix; RM_* are the newer QMK names)
RGB = [
 ("RGB_TOG", ["RM_TOGG", "RGB Toggle", "Lights Toggle"], "Turn the RGB lighting on or off", "RGB"),
 ("RGB_MODE_FORWARD", ["RGB_MOD", "RM_NEXT", "RGB Next", "Next Animation"], "Next lighting animation", "RGB▸"),
 ("RGB_MODE_REVERSE", ["RGB_RMOD", "RM_PREV", "RGB Previous", "Previous Animation"], "Previous lighting animation", "◂RGB"),
 ("RGB_SLD", ["RGB_MODE_PLAIN", "RGB_M_P", "RGB Solid", "Static Color"], "Static (solid) lighting mode; ZSA's code for 'show the layer colors' set in Oryx", "Solid"),
 ("RGB_VAI", ["RM_VALU", "RGB Brightness Up", "Light Up"], "Lighting brightness up", "☼+"),
 ("RGB_VAD", ["RM_VALD", "RGB Brightness Down", "Light Down"], "Lighting brightness down", "☼−"),
 ("RGB_HUI", ["RM_HUEU", "RGB Hue Up", "Hue Up"], "Lighting hue up", "Hue+"),
 ("RGB_HUD", ["RM_HUED", "RGB Hue Down", "Hue Down"], "Lighting hue down", "Hue−"),
 ("RGB_SAI", ["RM_SATU", "RGB Saturation Up", "Saturation Up"], "Lighting saturation up", "Sat+"),
 ("RGB_SAD", ["RM_SATD", "RGB Saturation Down", "Saturation Down"], "Lighting saturation down", "Sat−"),
 ("RGB_SPI", ["RM_SPDU", "RGB Speed Up", "Animation Speed Up"], "Animation speed up", "Spd+"),
 ("RGB_SPD", ["RM_SPDD", "RGB Speed Down", "Animation Speed Down"], "Animation speed down", "Spd−"),
]
for code, al, desc, leg in RGB:
    k(code, "rgb", desc, leg, al)

# ---- ZSA / Oryx specific
Z = [
 ("RGB", ["Set Color", "RGB Color", "Color"], "ZSA: set all LEDs to one color; the color is the action's `color` field (e.g. #ffab00)", "●"),
 ("TOGGLE_LAYER_COLOR", ["Layer Color Toggle", "Toggle Layer Colors"], "ZSA: turn the per-layer colors from Oryx on or off", "LyrClr"),
 ("LED_LEVEL", ["LED Level", "Status LED Level"], "ZSA: cycle the brightness of the status LEDs. Not yet seen in a real layout", "LED"),
 ("NAVIGATOR_INC_CPI", ["CPI Up", "Trackball Faster", "Navigator CPI Up"], "ZSA Navigator: raise trackball sensitivity (CPI)", "CPI+"),
 ("NAVIGATOR_DEC_CPI", ["CPI Down", "Trackball Slower", "Navigator CPI Down"], "ZSA Navigator: lower trackball sensitivity (CPI)", "CPI−"),
 ("NAVIGATOR_TURBO", ["Turbo", "Navigator Turbo"], "ZSA Navigator: hold for fast pointer movement. Not yet seen in a real layout", "Turbo"),
 ("NAVIGATOR_AIM", ["Aim", "Navigator Aim", "Precision"], "ZSA Navigator: hold for slow, precise pointer movement. Not yet seen in a real layout", "Aim"),
 ("DRAG_SCROLL", ["Drag Scroll", "Hold Scroll"], "ZSA Navigator: while held, the trackball scrolls instead of moving the pointer", "Scroll"),
 ("TOGGLE_SCROLL", ["Toggle Scroll", "Scroll Toggle"], "ZSA Navigator: toggle scroll mode for the trackball", "TgScrl"),
 ("MAC_MISSION_CONTROL", ["KC_MISSION_CONTROL", "KC_MCTL", "Mission Control"], "ZSA macOS key: Mission Control", "MsnCtl"),
 ("MAC_LAUNCHPAD", ["KC_LAUNCHPAD", "KC_LPAD", "Launchpad"], "ZSA macOS key: Launchpad. Not yet seen in a real layout", "Launch"),
 ("MAC_SPOTLIGHT", ["Spotlight"], "ZSA macOS key: Spotlight search. Not yet seen in a real layout", "Spot"),
 ("MAC_DND", ["Do Not Disturb", "DND"], "ZSA macOS key: toggle Do Not Disturb / Focus. Not yet seen in a real layout", "DND"),
 ("MAC_LOCK", ["Lock Screen", "Screen Lock"], "ZSA macOS key: lock the screen. Not yet seen in a real layout", "Lock"),
 ("MAC_DICTATION", ["Dictation", "Dictate"], "ZSA macOS key: start dictation. Not yet seen in a real layout", "Dict"),
]
for code, al, desc, leg in Z:
    k(code, "zsa", desc, leg, al)

# ---- quantum
Q = [
 ("QK_BOOT", ["QK_BOOTLOADER", "RESET", "Bootloader", "Boot", "Flash Mode"], "Reboot into the bootloader for flashing. Keep it on a hard-to-hit spot of a rarely used layer", "Boot"),
 ("QK_LLCK", ["QK_LAYER_LOCK", "Layer Lock", "LLock"], "Layer lock: keep the current layer on after releasing its MO/LT key; press again (or TO) to unlock. Useful on mouse and nav layers", "LLock"),
 ("CW_TOGG", ["QK_CAPS_WORD_TOGGLE", "Caps Word", "CapsWord"], "Caps Word: capitalise until a non-word key (space, punctuation). Not yet seen in a real layout", "CapsWd"),
 ("QK_REP", ["QK_REPEAT_KEY", "Repeat", "Repeat Key"], "Repeat the last key. Not yet seen in a real layout", "Repeat"),
 ("QK_AREP", ["QK_ALT_REPEAT_KEY", "Alt Repeat", "Alternate Repeat"], "Alternate repeat (the 'opposite' of the last key). Not yet seen in a real layout", "AltRep"),
 ("QK_DYNAMIC_TAPPING_TERM_PRINT", ["DT_PRNT", "Print Tapping Term"], "Type out the current tapping term in ms. Not yet seen in a real layout", "DT?"),
 ("QK_DYNAMIC_TAPPING_TERM_UP", ["DT_UP", "Tapping Term Up"], "Raise the tapping term by 5 ms. Not yet seen in a real layout", "DT+"),
 ("QK_DYNAMIC_TAPPING_TERM_DOWN", ["DT_DOWN", "Tapping Term Down"], "Lower the tapping term by 5 ms. Not yet seen in a real layout", "DT−"),
]
for code, al, desc, leg in Q:
    k(code, "quantum", desc, leg, al)

# ---- misc
k("KC_TRANSPARENT", "misc", "Transparent: falls through to the next active lower layer. A blank key in Oryx compiles to this too", "▽", ["KC_TRNS", "TRNS", "_______", "Transparent", "Trans", "▽"])
k("KC_NO", "misc", "No action: the key does nothing and blocks fall-through. Use it on gaming layers instead of transparent where the base has tap-hold keys", "", ["NO", "XXXXXXX", "Nothing", "Disabled"])

# ---- checks
names = {}
for e in E:
    for n in [e["code"]] + e["aliases"]:
        key = n.lower()
        if key in names and names[key] != e["code"]:
            sys.exit(f"duplicate alias {n!r}: {names[key]} and {e['code']}")
        if key in names and n != e["code"]:
            sys.exit(f"alias repeats own code or alias {n!r} in {e['code']}")
        names[key] = e["code"]
    if len(e["legend"]) > 6:
        sys.exit(f"legend too long: {e['code']} {e['legend']!r}")
# Older QMK spellings people still type (QMK removed them, old keymaps and docs have them).
LEGACY = {
    "KC_BSPC": ["KC_BSPACE"], "KC_PGDN": ["KC_PGDOWN"], "KC_LBRC": ["KC_LBRACKET"], "KC_RBRC": ["KC_RBRACKET"],
    "KC_SCLN": ["KC_SCOLON"], "KC_CAPS": ["KC_CAPSLOCK"], "KC_NUM": ["KC_NUMLOCK"], "KC_PSCR": ["KC_PSCREEN"],
    "KC_BSLS": ["KC_BSLASH"], "KC_SCRL": ["KC_SCROLLLOCK", "KC_SLCK"], "KC_BRIGHTNESS_UP": ["KC_BRMU"],
    "KC_BRIGHTNESS_DOWN": ["KC_BRMD"], "KC_DELETE": ["KC_DELT"], "KC_INSERT": ["KC_INS"],
}
taken = {n.lower() for e in E for n in [e["code"], *e["aliases"]]}
for e in E:
    for a in LEGACY.get(e["code"], []):
        if a.lower() not in taken:
            e["aliases"].append(a)
            taken.add(a.lower())
missing_legacy = set(LEGACY) - {e["code"] for e in E}
if missing_legacy:
    sys.exit(f"LEGACY names unknown codes: {missing_legacy}")

codes = {e["code"] for e in E}
missing = seen - codes
print("fixture codes missing:", missing)
print("count:", len(E))
cats = {}
for e in E: cats[e["category"]] = cats.get(e["category"], 0) + 1
print(cats)
json.dump(E, open(out, "w"), indent=2, ensure_ascii=False)
open(out, "a").write("\n")
