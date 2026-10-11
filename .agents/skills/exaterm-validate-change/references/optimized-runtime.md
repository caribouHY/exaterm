# Optimized Runtime Validation

For protocol connection setup, cancellation, timeouts or a Tauri command's async call-chain change, preserve the heap allocation boundary around deeply nested protocol futures.

Run the validation entrypoint's relevant code checks and:

```powershell
pnpm run tauri build --no-bundle
```

Launch `src-tauri/target/release/exaterm.exe` and exercise the affected command:

- Invalid input and a refused local connection must return errors without terminating the app.
- Verify successful connection and cancellation during setup when an appropriate endpoint is available.

Debug builds and unit tests do not replace this check. Report unperformed runtime scenarios. Read `Optimized Tauri Runtime Checks` in `docs/development/DEVELOPMENT_GUIDE.md` only when the rationale is needed; it explains Windows optimized-build stack risks.
