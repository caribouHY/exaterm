# ExaTerm Development Guide

This guide covers the common local development checks for ExaTerm.

## Setup

Install Rust and Node.js, then install pnpm:

```powershell
npm install -g pnpm@10.33.2
```

Install frontend dependencies from the repository root:

```powershell
pnpm install
```

## Formatting

Format React, TypeScript, CSS, JSON, Markdown, YAML, and Rust files:

```powershell
pnpm run format
```

Check formatting without changing files:

```powershell
pnpm run format:check
```

Frontend formatting is handled by Prettier. Rust formatting is handled by rustfmt through Cargo.

## Validation

Run frontend unit tests:

```powershell
pnpm run test:frontend
```

Build the frontend:

```powershell
pnpm run build
```

Run Rust tests:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Run Cargo validation commands from the repository root with an explicit manifest path.

### Optimized Tauri Runtime Checks

Rust async state machines can become significantly larger when protocol connection, timeout, and cancellation futures are nested. A Tauri command that directly awaits such a future embeds it in the generated IPC future and can exhaust the Windows main-thread stack in optimized builds even when debug builds and unit tests pass.

Keep deeply nested protocol connection futures behind a heap allocation boundary, such as `Box::pin`, at the Tauri command-dispatch boundary. Preserve that boundary when refactoring SSH, Telnet, Serial, or other long-running connection commands.

After changing protocol connection setup, cancellation, timeout handling, or the async call chain used by a Tauri command, build the optimized application without bundling:

```powershell
pnpm run tauri build --no-bundle
```

Launch `src-tauri/target/release/exaterm.exe` and invoke the affected command. For connection changes, verify at minimum that invalid input and a refused local connection return errors without terminating the application. When an appropriate test endpoint is available, also verify a successful connection and cancellation during setup. Do not treat debug packaging alone as proof that the optimized runtime is safe.

## Branches and Commits

When editing code, create a working branch from the latest `dev` branch. Do not commit directly to `dev`; merge changes through pull requests.

Write commit messages in English and prefix them with the change type:

```text
feature: add SSH profile import
fix: preserve terminal size on resize
refactor: simplify config validation
docs: update development guide
test: add config parser tests
chore: update build dependencies
```

Keep commits reviewable and focused. As a rule, each commit should contain one logical change, and unrelated changes should not be mixed into the same commit.

## Pull Request Checks

Open pull requests against the `dev` branch.

Pull requests run the GitHub Actions CI workflow on `windows-latest`. The workflow installs dependencies, checks formatting, builds the frontend, and runs Rust tests.

## Updater Signing

The updater uses a dedicated Tauri signing key. This verifies that an update was produced by the project; it is separate from Windows Authenticode signing.

- Keep the private key outside the repository and never commit it.
- Keep the key password in a protected credential store.
- Do not replace or lose the key after shipping an updater-enabled release. Existing installations will reject updates signed with a different key.
- Commit only the public key in `src-tauri/tauri.conf.json`.

The release workflow expects these GitHub Actions secrets:

- `TAURI_SIGNING_PRIVATE_KEY`: the full private-key file content
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the private-key password

Release builds merge `src-tauri/tauri.release.conf.json`, which enables signed updater artifacts without requiring signing secrets for normal debug builds. Before publishing a draft release, verify that it contains the MSI and NSIS installers, their `.sig` files, and `latest.json`.

The first updater-enabled release must be installed manually. Use a later stable release to verify the complete in-app update flow.
