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
