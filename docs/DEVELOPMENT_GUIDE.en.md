# ExaTerm Development Guide

This guide covers the common local development checks for ExaTerm.

## Setup

Install Rust and Node.js, then install frontend dependencies from the repository root:

```powershell
npm install
```

## Formatting

Format React, TypeScript, CSS, JSON, Markdown, YAML, and Rust files:

```powershell
npm run format
```

Check formatting without changing files:

```powershell
npm run format:check
```

Frontend formatting is handled by Prettier. Rust formatting is handled by rustfmt through Cargo.

## Validation

Build the frontend:

```powershell
npm run build
```

Run Rust tests:

```powershell
cd src-tauri
cargo test
```

Run Cargo commands from `src-tauri` unless the command uses an explicit manifest path.

## Pull Request Checks

Pull requests run the GitHub Actions CI workflow on `windows-latest`. The workflow installs dependencies, checks formatting, builds the frontend, and runs Rust tests.
