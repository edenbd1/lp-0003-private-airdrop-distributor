# LP-0003 airdrop claim — Basecamp app

A Logos Basecamp surface for claiming a private airdrop allocation from inside
Basecamp. Built as a Qt6/QML plugin: `ClaimPlugin` implements `IComponent` with
`Q_PLUGIN_METADATA`, and a `ClaimBridge` shells out to the local `airdrop` CLI
(`crates/airdrop-cli`) to build a recipient's claim arguments, which are then
submitted with `spel`.

```
app/
├── CMakeLists.txt      # dual-path build (framework + standalone)
├── module.json         # Basecamp module manifest
├── qml/Main.qml        # the claim pane
└── src/
    ├── plugin.{h,cpp}      # ClaimPlugin: IComponent + QQuickWidget host
    ├── claim_bridge.{h,cpp} # QML bridge over the airdrop CLI
    └── metadata.json
```

## Build paths

### Framework build (production `.lgx`)

When `LOGOS_MODULE_BUILDER_ROOT` points at a `logos-module-builder` checkout
(the Nix dev shell), the build includes `LogosModule.cmake` and the
`logos_module()` macro wires Qt + the Logos SDK + LGX packaging into an
installable `.lgx`:

```bash
LOGOS_MODULE_BUILDER_ROOT=/path/to/logos-module-builder cmake -B build
cmake --build build          # produces lp_0003_airdrop.lgx
```

The packaged `.lgx` **is committed** at `app/lp-0003-airdrop.lgx` (built with the
command above; the module-builder toolchain is not in this repository's CI, so the
asset is committed rather than rebuilt on every push). It ships both a
`darwin-arm64` and a `linux-amd64` variant; other platforms can rebuild with the
command above or use the cross-platform CLI.

### Standalone build (developer preview)

With only Qt6 on the path, the plugin builds as a normal Qt plugin for local
preview:

```bash
cmake -B build -DCMAKE_PREFIX_PATH="$(brew --prefix qt6)"
cmake --build build
```

## Loading in Basecamp

Drop the built `lp_0003_airdrop.lgx` into Basecamp's user-plugins directory
(`~/Library/Application Support/Logos/LogosBasecampDev/plugins/` on macOS);
Basecamp's PluginLoader picks it up on next launch.


## Packaged asset

`app/lp-0003-airdrop.lgx` (1.5 MB, SHA-256 `42649f5fc12279d3ddfc45a0e7db781e407b6aae5e933378184ac8f0f0ccaab3`) is the
packaged module, built with the Logos module builder and the `lgx` tool
(`logos-package`). It ships **two variants**, `darwin-arm64` and `linux-amd64`,
each carrying the plugin (`.dylib`/`.so`), the QML view, the module metadata, and
the `airdrop` CLI the bridge drives, so it loads in Basecamp on both macOS
(arm64) and Linux (x86_64). Drop it into Basecamp's user-plugins directory to load
it; `lgx manifest app/lp-0003-airdrop.lgx` lists the variants.
