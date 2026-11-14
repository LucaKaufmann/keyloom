# 06: Live keyboard integration (Keymapp)

Optional features that act on the physical keyboard. Everything else in keyloom works without them.

## Transport

Keymapp's local gRPC API (`api.KeyboardService`, [keymapp.proto](https://github.com/zsa/kontroll/blob/main/proto/keymapp.proto)).
Requires Keymapp ≥ 1.3.2 running with the API enabled (Keymapp → Settings → API). Where keyloom looks:

1. `KEYMAPP_PORT` → TCP `127.0.0.1:<port>` (Windows default: 50051).
2. `KEYMAPP_SOCKET` → that Unix socket (same override as ZSA's kontroll).
3. macOS `~/Library/Application Support/.keymapp/keymapp.sock`, or the Mac App Store build's sandboxed
   `~/Library/Containers/io.zsa.keymapp/Data/Library/Application Support/.keymapp/keymapp.sock`;
   Linux `<config dir>/.keymapp/keymapp.sock`.

Implementation: `keyloom-cli/src/keymapp.rs`, a small client of its own (HTTP/2 via `h2`, the few protobuf
messages encoded by hand) because the `kontroll` crate needs `protoc` at build time. Calls: `GetStatus`,
`GetKeyboards`, `ConnectAnyKeyboard`, `SetLayer`, `UnsetLayer`, `SetRGBLed`, `SetRGBAll`. A reply with
`success = false` is an error ("Keymapp couldn't …"), as is a gRPC status other than 0.
Tested against a fake Keymapp (`keymapp::fake`); **not yet against the real one** (PLAN, Status).

## Features

| Feature | Calls |
| --- | --- |
| `keyloom status` | `GetStatus` (+ `GetKeyboards` when none is connected), flashed-layout detection |
| highlight keys (`--show`, `live highlight`, MCP `keyboard(highlight)`) | `SetLayer(layer)` if asked, `SetRGBAll(black)`, `SetRGBLed(index, color)` per key |
| show a layer (`show --show`, `live layer`, MCP `keyboard(show_layer)`) | `SetLayer(n)`; layer 0 = `UnsetLayer(current)` until the base layer shows |
| guide stepping (03) | lights the step's key(s); the keyboard's layer is not switched |
| reset (`live reset`, MCP `keyboard(reset)`) | `SetRGBAll(black, sustain 1 ms)` (the firmware's own colors come back, as kontroll does), `UnsetLayer(the layer keyloom set)`, `SetLayer(the layer the keyboard was on before)` |

## Rules

- **What keyloom changed is remembered** in `$KEYLOOM_HOME/cache/live.json` (the layer it switched on, the
  layer the keyboard was on before, whether it changed the lights), because the change and the reset are often
  different processes. Reset undoes exactly that; a layer the user switched themselves is left alone.
- **One-shot commands leave their result on the keyboard** (`--show`, `keyloom live highlight|layer`), print
  the `keyloom live reset` hint, and exit 1 if Keymapp refused or isn't reachable (`--show` only reports it).
  `guide --step` restores when it ends, on `q` and on Ctrl-C; the MCP server restores its own changes on exit.
- **A draft's layer numbers aren't the keyboard's.** Showing a layer of a draft (or of an older revision)
  switches the keyboard to the matching layer of the revision on the keyboard: the flashed one when its USB
  serial names an imported revision, otherwise the newest imported one. A layer that revision doesn't have
  can't be shown.
- **LED index = key index** (checked on a Voyager with Keymapp 1.3.7, PLAN, Status).
- **Keymapp just after connecting** refuses commands for a moment ("keyboard requires an updated firmware")
  and can refuse to connect just after a disconnect; keyloom retries both briefly.
- **Keymapp must hold the keyboard.** With no keyboard connected (e.g. after a replug) keyloom asks Keymapp to
  connect one and reports it (`keymappReconnected`). An app that opens the keyboard's HID interface itself can
  keep Keymapp from connecting.
- **No surprises from agents.** The MCP `keyboard` tool says in its description that it changes the
  keyboard's lights/layer; its `reset` action is always available.

## Detecting the flashed layout

In order:
1. Keymapp `GetStatus.firmware_version`, if it has the form `<layoutId>/<revisionId>` (unverified).
2. USB serial number of the ZSA device (vendor 0x3297); Oryx firmware reports `<layoutId>/<revisionId>`
   (confirmed in Oryx's generated `config.h`). macOS via `ioreg -p IOUSB -l`; Linux via sysfs.
3. Otherwise unknown.

`keyloom status` shows it and whether that revision is imported.
