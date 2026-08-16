# Interactive Web Constructor Interface

This document describes the design, setup, and roadmap of the browser-based interactive constructor interface for `ingrid_core`.

---

## 1. Overview

The web interface provides an interactive, live assisted-fill construction environment. Constructors can define arbitrary grid slots (including hexagonal, 3D, or standard grids) and type letters into cells. The Wasm-compiled Rust solver runs the AC-3 constraint propagation algorithm on every keystroke, instantly highlighting invalid paths and filtering candidate word lists for all slots.

---

## 2. How to Build and Run

### 1. Build the WebAssembly Package
From the root of the repository, compile the Rust engine into a Web-compatible WebAssembly module:
```bash
wasm-pack build --target web
```

### 2. Launch a Static File Server
Start any simple local HTTP server from the root of the repository. For example, using Python:
```bash
python3 -m http.server 8000
```

### 3. Open the Interface
Navigate to the web interface in your browser:
[http://localhost:8000/www/](http://localhost:8000/www/)

---

## 3. What Has Been Done

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
* **`www/index.html`**: A dark-theme CSS/HTML layout containing panels for entering slot configurations and browsing valid candidate words.
* **`www/index.js`**: Automatically fetches the default dictionary `spreadthewordlist.dict` from the resources folder at startup, instantiates the WebAssembly package, sets the minimum score constraint to 50, renders the cell board, and binds interactive actions.
* **Word-by-Word Board Representation**: Displays cell input squares grouped horizontally as individual slot rows. Focus events highlight matching cells in all crossing slots and select the slot's candidates in the sidebar. Typing in a cell dynamically synchronizes its value to all intersecting slots on the fly.
* **Smart Focus Navigation (MCV Heuristic)**: After selecting a candidate word and executing AC-3, the UI automatically identifies the next incomplete slot with the fewest remaining candidate options, focuses it, and highlights its first empty cell.
* **Backspace Cell Navigation**: Pressing Backspace clears the current letter, syncs the change to all crossing cells, re-runs AC-3 propagation, and shifts input focus to the previous cell within the active slot row.
* **Auto-Advance Cell Navigation**: Typing a letter in a cell automatically synchronizes it across crossing slots, runs AC-3, and shifts focus to the next cell in the active slot row (stopping at the end of the slot).
* **Custom Dictionary Upload**: Users can upload `.dict` or `.txt` word lists directly in the left panel. The browser reads the file locally and instantly re-initializes the solver.
* **Minimum Word Score Filter**: The minimum score threshold can be customized via a numeric input. Updating the score automatically updates candidate availability and runs the solver without reload.

---

## 4. What Remains to Do / Roadmap

1. **Web Worker Integration**:
   * Move the `WasmSolver` instantiations and `run_ac3` calls into a Web Worker.
   * This prevents CPU-heavy dictionary filtering and constraint propagation loops from blocking the main UI thread (avoiding page lag).

2. **Performance Optimization for Large Dictionaries**:
   * Improve parsing speeds of large dictionary files (e.g., standard dictionaries with >100,000 words) inside the `load_dictionary` function.
