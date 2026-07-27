# Compatibility fixtures

RC1 treats this directory as a release input, not as a generated test asset.
Each sample must contain only synthetic, redistributable data and be recorded
in `manifest.json` with its producer version and SHA-256. Do not add user
archives, password material, or copyrighted test data.

The required producer matrix is deliberately checked outside the normal CI
loop because several tools are proprietary or platform-specific. A public RC
cannot claim the matrix as passed until all files listed in the manifest are
present and validated by `scripts/verify-compat-fixtures.ps1`.
