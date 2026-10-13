# Third-party references and dependencies

Omen Hub++ contains original Rust code. OmenMon/OmenHwCtl are references; their implementations have not been copied or linked.

Direct Rust dependencies are `serde`, `serde_json`, `wmi`, `windows`, `windows-sys`, `libloading`, `sha2` and `quick-xml`. The exact dependency graph is locked in `Cargo.lock`; review each dependency's license and redistribution requirements before distributing release packages. This list is not a complete set of notices for every transitive dependency.

[UXTU](https://github.com/JamesCJ60/Universal-x86-Tuning-Utility) and [NvAPIWrapper](https://github.com/falahati/NvAPIWrapper) are local research references for feature scope and private-contract investigation. Their code is not copied or linked into this Rust runtime. Private control tables still require exact driver evidence and full restore validation; a matching structure size alone is insufficient.

The NVIDIA FFI uses public ABI definitions and function identifiers from [NVIDIA/nvapi](https://github.com/NVIDIA/nvapi) and the [NVML documentation](https://docs.nvidia.com/deploy/nvml-api/latest/index.html). NVIDIA's SDK header repository uses MIT; driver DLLs are loaded from the installed Windows driver and are not bundled here.

NVAPI header attribution: Copyright (c) 2019–2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved. SPDX-License-Identifier: MIT. The MIT permission and warranty text is provided in `LICENSE`.

[RyzenAdj](https://github.com/FlyGoat/RyzenAdj) is a planned optional native dependency, not included in this build. Its LGPL-3.0 terms, driver requirements and CPU-family support must be reviewed before integration or redistribution. No HP assemblies, HP drivers, WinRing0 binaries or decompiled code are distributed.
