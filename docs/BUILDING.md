# Building and packaging

## Prerequisites

- Windows x64, Rust's `x86_64-pc-windows-msvc` target, Visual Studio C++ build tools and Windows SDK.
- The workspace declares Rust 1.85/edition 2024. Recent recorded checks used Rust/Cargo 1.99.0; CI uses stable Rust. The declared minimum is not a separate recorded MSRV qualification.
- Windows PowerShell 5.1 (`powershell.exe`) is sufficient for the scripts. PowerShell 7 (`pwsh`) is not required.

Use the committed `Cargo.lock`. Build preparation can download dependencies; the app runtime contains no network client, updater or telemetry.

## Optional CPU modules

Before building CPU hardware features, stage the pinned signed modules and their corresponding source/license:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\prepare-cpu-module.ps1
```

This prepares ignored `.tools/pawnio/` build inputs. It does not install, start or stop a driver. `platform/build.rs` checks the module hashes before embedding them. Without `AMDFamily17.bin`, measured CPU hardware telemetry is unavailable. Without `RyzenSMU.bin`, the SMU tuning backend is unavailable.

Runtime CPU access additionally requires the already installed/running, exactly verified PawnIO 2.2.0 driver and administrator permission for the relevant helper. The application does not install it. See [CPU sensors](CPU_SENSORS.md) and [third-party notices](../THIRD_PARTY.md).

## Checks and executable

```powershell
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --release --locked
```

The executable is `target/release/omen-hub-pp.exe`. Unit tests and the standalone guardian process fixtures use models, temporary files and owned test processes; they do not perform real service or hardware writes.

Additional isolated checks:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\test-installer-recovery.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\test-power-lifecycle-evidence.ps1
```

These check installer guards and lifecycle evidence classification. They neither install the app nor request standby/reboot. Other scripts named `check-*`, `smoke-*` or `measure-*` have different effects; inspect them before use. For example, GPU/system-control qualification scripts deliberately change and restore hardware settings, and native sensor tests can request UAC. Their names alone do not imply read-only behavior.

## Package

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

The script creates a fresh versioned staging directory under `target/package-staging/`, copies the current package to `dist/`, and writes:

- `target/OmenHubPlusPlus-<version>-x64.zip`;
- `target/OmenHubPlusPlus-<version>-SHA256SUMS.txt`;
- packaged `build-info.json` with source commit, dirty state, executable hash and CPU-module flags.

The archive includes the EXE, installers, recovery/support notes and third-party licenses. Embedded CPU modules require their corresponding source archive and LGPL notice. Existing unrelated files in `dist/` are not included in the fresh archive. The checksum file records the ZIP's SHA-256; it detects mismatches and is not a publisher signature.

The package contains selected guides, not the entire documentation tree. Additional linked topics and historical records are available in the [repository documentation](https://github.com/Ishtvr/Omen-Hub-debloat/tree/main/docs).

Release notes come from `docs/releases/<version>.md`. A development package may lack them; a public candidate must supply them before strict package checking:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\check-release-package.ps1 -Package 'target\OmenHubPlusPlus-<version>-x64.zip' -Version '<version>' -ExpectedCommit '<full-commit-sha>' -RequireCleanSource
```

This reads the ZIP without launching or installing it. Verify the exact source revision and documentation scope before publishing. A Cargo release build is not itself a public release, installed update or new hardware qualification.

## Repository layout and local files

| Path | Purpose |
| --- | --- |
| `crates/core/` | Platform-independent settings, editing rules and transaction policies |
| `crates/platform/` | Windows adapters and persistent stores |
| `crates/app/` | Native UI, tray, helpers and CLI |
| `scripts/` | Build/install tools and explicitly scoped checks |
| `docs/` | Current guides, model evidence, research plans and dated history |
| `.impeccable/` | Versioned design configuration and intent briefs; no runtime web UI |

`target/`, `dist/`, `.local/`, `.tools/` and local skills under `.agents/` are ignored. Preserve current research evidence, pinned modules, corresponding sources/licenses and needed proof programs. Compiler caches are reproducible; those inputs and records are not interchangeable with disposable caches. Never publish proprietary DLLs, drivers, decompiled sources, personal dumps or secrets.
