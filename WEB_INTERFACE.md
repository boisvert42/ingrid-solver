# Interactive Web Constructor Interface

This document describes the design, setup, and roadmap of the browser-based interactive constructor interface for `ingrid_core`.

---

## 1. Overview

The web interface provides an interactive, live assisted-fill construction environment. Constructors can define arbitrary grid slots (including hexagonal, 3D, or standard grids) and type letters into cells. The Wasm-compiled Rust solver runs the AC-3 constraint propagation algorithm on every keystroke, instantly highlighting invalid paths and filtering candidate word lists for all slots.

---

## 2. What Has Been Done

### A. Rust Engine WebAssembly Bindings (`src/wasm.rs`)
* Created the `WasmSolver` struct exposed via `wasm-bindgen`.
* **Topology Parsing**: Parses whitespace/newline/semicolon delimited slot definitions, maps them to flat 1D cell arrays, and computes crossing constraints.
* **Memory-Based Loading**: Exposes `load_dictionary` which parses dictionary files in-memory to populate the solver without requiring static file compiles or filesystem access.
* ** लाइव AC-3 passes**: Exposes `run_ac3` which takes the current board fill as a JSON string, performs fast arc-consistency checking, and returns the lists of remaining valid options for all slots.

### B. Compilation Environment Configurations
* Added `wasm-bindgen` and `serde_json` dependencies to `Cargo.toml`.
* Configured crate-types in `Cargo.toml` to support compiled WebAssembly target: `crate-type = ["cdylib", "rlib"]`.
* Enabled the `wasm_js` feature flag for `getrandom` in `Cargo.toml` to enable random number generation inside browser runtimes.

### C. Frontend Board & UI Application (`www/`)
* **`www/index.html`**: A dark-theme CSS/HTML layout containing panels for entering slot configurations, inputting custom dictionaries, and browsing valid candidate words.
* **`www/index.js`**: Integrates the compiled WebAssembly package, populates interactive grid input cards, and binds autofill actions (clicking a candidate fills the slot and re-runs AC-3).

---

## 3. What Remains to Do / Roadmap

1. **Web Worker Integration**:
   * Move the `WasmSolver` instantiations and `run_ac3` calls into a Web Worker.
   * This prevents CPU-heavy dictionary filtering and constraint propagation loops from blocking the main UI thread (avoiding page lag).

2. **Visual Grid Board Enhancements**:
   * Instead of a flat list of cell input cards, parse the topological specifications to render visual boards (e.g. rendering hexagonal honeycombs for hexagonal slots, or interactive 2D grid meshes).

3. **Performance Optimization for Large Dictionaries**:
   * Improve parsing speeds of large dictionary files (e.g., standard dictionaries with >100,000 words) inside the `load_dictionary` function.
