# Project Instructions

- Do not use browser automation tools for visual verification in this project.
- For UI and layout changes, verify with code inspection and build/type checks unless the user explicitly asks for browser-based testing.
- If a local dev server is useful, provide the URL for the user to inspect manually instead of opening it with browser automation.
- Before creating or moving a release tag, synchronize the release version in all of these files: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`.
- Before pushing a release tag, inspect `.github/workflows/release.yml` and update the release body/changelog so it matches the tag being published.
- For release tags, verify with `rg -n "<previous-version>|<new-version>" package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json .github/workflows/release.yml` before pushing.
