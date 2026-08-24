# LP-0003 Basecamp app

A Logos Basecamp surface for the private airdrop: point at a published
distribution, open your row from the encrypted bundle, and build the claim
arguments to submit on the privacy-preserving path.

Every button runs an `airdrop` subcommand through `ClaimBridge`, so the GUI and
the chain compute the same commitments from the same code. There is no second
implementation to drift.

**The app never holds a recipient secret on disk.** A secret is passed to the CLI
process and never written out by this plugin.

## Building

### With the Logos module builder

Inside the `logos-module-builder` dev shell, where `$LOGOS_MODULE_BUILDER_ROOT`
is set, `LogosModule.cmake` wires up Qt, the SDK, and packaging:

```bash
cd app && cmake -B build -S . && cmake --build build
```

### Standalone (for QML iteration)

Plain Qt6, no Logos stack. Produces `build/lp_0003_airdrop.<so|dylib>`. Requires
Qt 6 with `Core Gui Widgets Quick QuickWidgets Qml`.

```bash
cd app && cmake -B build -S . && cmake --build build
```

### Which Qt — this is load-bearing

Build against **Qt 6.9.x or older**, not against whatever Qt is newest.

Qt refuses to load a plugin whose minor version is above the host's, and Logos
Basecamp 0.2.2 ships Qt 6.9.2. A plugin built against Homebrew's current Qt
(6.11.1) is rejected outright, with nothing in the UI to explain it — the app
tile does nothing, and the reason appears only on Basecamp's stderr:

```
Failed to load UI module "lp-0003-airdrop" :
  "The plugin ... uses incompatible Qt library. (6.11.0) [release]"
```

The committed `darwin-arm64` variant is built against Qt 6.9.2, obtained without
touching the system Qt:

```bash
python3 -m venv /tmp/aqt && /tmp/aqt/bin/pip install aqtinstall
/tmp/aqt/bin/aqt install-qt mac desktop 6.9.2 clang_64 --outputdir /tmp/Qt

cd app && cmake -B build -S . -DCMAKE_PREFIX_PATH=/tmp/Qt/6.9.2/macos -DCMAKE_BUILD_TYPE=Release
cmake --build build
```

Official Qt builds reference their frameworks as `@rpath/QtCore.framework/...`,
which resolves against the Qt that Basecamp bundles; a Homebrew build hardcodes
`/opt/homebrew/opt/qtbase/lib/...` and fails anywhere else. `QtConcurrent` is
**not** bundled by Basecamp, so it is deliberately not linked. The `linux-amd64`
variant builds against Debian bookworm's Qt 6.4.2, which is below 6.9 and fine.

Check a build before shipping it:

```bash
otool -L build/lp_0003_airdrop.dylib | grep -c /opt/homebrew   # must be 0
otool -L build/lp_0003_airdrop.dylib | grep QtCore             # must say 6.9.x
```

### The plugin interface is ABI-critical

`src/plugin.h` declares `IComponent` locally so the standalone path needs no SDK
header. Two things in that declaration are not free choices:

- the interface string must be `com.logos.component.IComponent`. It is what
  `qobject_cast<IComponent*>` compares across the plugin boundary; a private IID
  makes the cast return null and Basecamp logs *"Plugin does not implement
  IComponent"*.
- the virtual functions must be exactly `~IComponent`, `createWidget`,
  `destroyWidget`, in that order. An extra virtual — a `name()` accessor, say —
  shifts every later vtable slot, so the host calls the wrong function through a
  pointer that cast successfully.

Both were verified against LogosBasecamp 0.2.2.

## Using it

1. Point **Distribution folder** at a demo distribution directory (built by
   `airdrop demo-distribution`). The `airdrop` binary is found automatically: the
   plugin resolves its own path with `dladdr` and uses the CLI shipped beside it
   in the package, so a freshly installed `.lgx` works without configuration.
2. Pick a recipient index and press **Build claim** — the bridge opens the row
   and prints the nullifier, marker seed, and allocation, writing the SPEL claim
   arguments to a file. Submit them with `spel` on the privacy-preserving path.

The surface shows a nullifier and a marker address, never an address — because
that is all the chain records.

## Packaged asset

`app/lp-0003-airdrop.lgx` (1.2 MB, SHA-256 `3d129c9ac58a2f8c20343fc84daae2083061e7f20e990d8bdcab8884d8b54581`)
is the packaged module. It carries **two variants** — `darwin-arm64` and
`linux-amd64` — each with the plugin library, the QML view, the module metadata,
and the `airdrop` CLI the bridge drives. Basecamp selects the one matching the
host.

Two variants rather than one because the evaluator reviews on a Linux VPS (see
logos-co/lambda-prize#67); a macOS-only package is one they cannot open.

### Installing it

```bash
lgx extract app/lp-0003-airdrop.lgx --variant darwin-arm64 --output /tmp/x
DST=~/Library/Application\ Support/Logos/LogosBasecamp/plugins/lp-0003-airdrop
mkdir -p "$DST" && cp -R /tmp/x/darwin-arm64/. "$DST/"
printf darwin-arm64 > "$DST/variant"
tar xzOf app/lp-0003-airdrop.lgx manifest.json > "$DST/manifest.json"
```

On Linux the directory is `~/.local/share/Logos/LogosBasecamp/plugins/`, and the
variant is `linux-amd64`. Restart Basecamp; the module appears in the left rail.

### How this was verified against Basecamp 0.2.2

Shipping a `.lgx` is not the same as it loading, so it was run on the real app.
Installed as above on LogosBasecamp 0.2.2 (official macOS arm64 release), the
module loads:

```
App launcher clicked: "lp-0003-airdrop"
Loading UI module: "lp-0003-airdrop"
MainContainer: Added plugin dock to WorkspaceArea: "lp-0003-airdrop"
Successfully loaded UI module: "lp-0003-airdrop"
```

The **Private Airdrop Claim** surface renders and is usable. Driven end to end: a
distribution of 8 recipients generated with the CLI shipped inside the package,
then **Build claim** produced nullifier `bf28b71b…`, marker seed `5efad061…`, and
wrote `claim_gui.args` — with the CLI field left empty, so the `dladdr` resolution
of the packaged `airdrop` binary works, and the args matched the CLI's own output
for the same recipient byte for byte. The tile appears only because the manifest
carries `type: "ui"`; `lgx add` leaves it empty, so `package-lgx.py` folds it in
(see below), and with an empty `type` the module is invisible.

From a clean clone without a GUI, the load can also be reproduced with
`QPluginLoader` against Basecamp's Qt 6.9.2: the committed `darwin-arm64` dylib
reports IID `com.logos.component.IComponent`, `load()` succeeds, and
`qobject_cast<IComponent*>` returns a valid instance, where a Homebrew build fails
with *"incompatible Qt library (6.11.0)"*.

Verify the package matches its own manifest:

```bash
python3 scripts/package-lgx.py --verify app/lp-0003-airdrop.lgx
```

### Rebuilding the Linux variant

The macOS half builds natively (above). The Linux half builds in Docker:

```bash
./scripts/build-linux-variant.sh                                   # ARCH=arm64 for linux-arm64
python3 scripts/package-lgx.py --add-variant linux-amd64:.linux-variant
```

That produces the ELF plugin and a Linux `airdrop`, then folds both into the
package. The CLI matters: the bridge shells out to `airdrop`, so a variant
carrying a Linux plugin next to a macOS binary would load and then fail on the
first button press.

### How it was packaged

**With the real `lgx`** from
[`logos-co/logos-package`](https://github.com/logos-co/logos-package).
`scripts/package-lgx.py` finds it at `$LGX_BIN`, then
`~/logos/src/logos-package/build/lgx`, then on `PATH`, and calls `lgx create` /
`lgx add`. Metadata is folded into the manifest afterwards, exactly as
`nix-bundle-lgx`'s `bundle.sh` does, because `lgx add` never reads
`metadata.json` and would otherwise leave author, description, type and category
empty — which makes the module invisible in Basecamp. **Without `lgx`**, the
script falls back to writing the package itself, and refuses to do so unless its
transcription of `logos-package`'s hash scheme still reproduces the manifest of a
real package. The root hash for this build is
`41c9dce8f2e7fc8449fc12ad6b154938d66274a985fa0b77d1dd7d303a256353`.

```bash
lgx manifest app/lp-0003-airdrop.lgx
python3 scripts/package-lgx.py --verify app/lp-0003-airdrop.lgx
```

## Files

| File | What |
|---|---|
| `metadata.json`, `module.json` | The module manifest, kept in sync at configure time. Basecamp's package manager reads the former; the λPrize validator looks for the latter |
| `src/plugin.{h,cpp}` | The Qt plugin: hosts the QML scene, exposes the bridge |
| `src/claim_bridge.{h,cpp}` | Shells out to `airdrop`, resolving the CLI shipped beside it via `dladdr` |
| `qml/Main.qml` | The surface |


## What only shows up with two modules installed

**Two modules that both register `qrc:/qml/Main.qml` render each other.** Qt's
resource system is process-global, so whichever registers first wins for both:
with two of these installed together, one tile showed the other module's
UI. Each loaded fine on its own, and `QPluginLoader::load()` was happy in both
cases — the collision is invisible until a second module is present. The resource
prefix is now this module's own name, which cannot collide. Verified in Basecamp
0.2.2 with five modules installed at once, each opened in turn.

A sibling module that talks to a sequencer over `QNetworkAccessManager` has a
second, harsher failure on first click — Qt's macOS proxy lookup asks PCRE2 to
JIT-compile a regex, and Basecamp runs hardened without
`com.apple.security.cs.allow-jit`, so the host dies with `SIGTRAP`. This module
shells out to a CLI and never uses Qt networking, so it is not affected.
