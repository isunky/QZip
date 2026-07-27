# Dependency policy

QZip keeps runtime dependencies small and purpose-specific. Core archive crates
must not depend on Tauri, and UI components must not depend on Tauri APIs.

Before adding a dependency, confirm its license, maintenance status, and
whether a platform-native or standard-library capability already exists.

## M1 additions

- `async-trait` keeps `ArchiveBackend` object-safe while exposing asynchronous
  operations to the desktop shell and CLI.
- `secrecy` wraps passwords so normal debug output does not reveal values.
- `tokio` and `tokio-util` run the Sidecar asynchronously and provide
  cancellation tokens.
- `clap` and `rpassword` provide the developer-only CLI and hidden password
  prompt; passwords are never accepted as a CLI argument.
