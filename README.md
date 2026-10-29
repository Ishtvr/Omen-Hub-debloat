# Omen Hub++

A lightweight, native Windows control tool for HP OMEN laptops, written in Rust. Built to bring hardware monitoring and tuning into one clean, offline app—without ads, telemetry, accounts or a background updater.

**In development · `0.9.0-dev.2` · Windows x64**

## Features

- GPU power, temperature and clock monitoring alongside CPU telemetry.
- GPU/VRAM clock offsets and an interactive voltage/frequency curve editor.
- CPU power limits, Curve Optimizer, PBO and Windows power settings.
- Custom presets with optional application at Windows sign-in.
- Fan curves, maximum cooling and four-zone keyboard lighting.
- A native dark interface in German and English, with system tray integration.

**Hardware support is currently limited to the tested HP OMEN 16-ap0xxx, board 8E35, BIOS F.13, Ryzen 9 8940HX and RTX 5070 Laptop configuration.** Private GPU controls require the qualified NVIDIA 617.14 or 617.42 implementation. CPU hardware features require the separately installed, verified PawnIO driver.

This is an experimental application. Full replacement of HP's background services, real reboot/sleep recovery and freely adjustable GPU TGP are still in progress. See the [hardware matrix](docs/HARDWARE_MATRIX.md) and [roadmap](docs/ROADMAP.md). The first beta was withdrawn; no replacement release is currently published.

## Build

Install Rust for Windows x64 with the MSVC build tools and Windows SDK, then run:

```powershell
cargo build --release --locked
.\target\release\omen-hub-pp.exe
```

CPU modules need an additional preparation step. See [building and packaging](docs/BUILDING.md) for full instructions.

## Documentation

[Getting started](docs/GETTING_STARTED.md) · [Hardware support](docs/HARDWARE_MATRIX.md) · [Recovery](docs/RECOVERY.md) · [All documentation](docs/README.md)

## License and disclaimer

[MIT](LICENSE), with separately licensed [third-party components](THIRD_PARTY.md). This software is under active development. **Use it at your own risk. To the extent permitted by law, the author and contributors accept no liability for damage, data loss or other consequences of its use.** No warranty is provided; see the license terms.
