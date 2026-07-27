# M0 architecture boundaries

React UI flows into the desktop application shell, then future Tauri adapters,
future application services, and future archive crates.

packages/ui contains visual tokens and stateless primitives only. The five Rust
crates establish the future domain boundaries and intentionally contain no
archive implementation in M0. The desktop crate is the only crate allowed to
depend on Tauri.
