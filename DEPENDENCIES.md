# Dependency policy

QZip keeps runtime dependencies small and purpose-specific. Core archive crates
must not depend on Tauri, and UI components must not depend on Tauri APIs.

Before adding a dependency, confirm its license, maintenance status, and
whether a platform-native or standard-library capability already exists.
