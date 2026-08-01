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

The packaged `.lgx` is not committed here because that toolchain is not in this
repository's CI environment; it is produced by the command above, exactly as the
LP-0005 app's `.lgx` was.

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
