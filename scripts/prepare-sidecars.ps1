$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$manifestPath = Join-Path $repositoryRoot "src-tauri\Cargo.toml"
$cargoTargetDirectory = Join-Path $repositoryRoot "src-tauri\target\sidecars"
$targetDirectory = Join-Path $cargoTargetDirectory "release"
$binaryDirectory = Join-Path $repositoryRoot "src-tauri\binaries"

$hostLine = rustc -vV | Select-String "^host:"
if (-not $hostLine) {
    throw "Unable to determine the Rust host target."
}
$targetTriple = ($hostLine.Line -split ":", 2)[1].Trim()

New-Item -ItemType Directory -Force -Path $binaryDirectory | Out-Null

$extension = if ($IsWindows -or $env:OS -eq "Windows_NT") { ".exe" } else { "" }
$binaries = @("exaterm-mcp", "exaterm-cli")

foreach ($binary in $binaries) {
    cargo build --release --target-dir $cargoTargetDirectory --manifest-path $manifestPath --bin $binary
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to build ExaTerm sidecar binary: $binary"
    }

    $source = Join-Path $targetDirectory "$binary$extension"
    $destination = Join-Path $binaryDirectory "$binary-$targetTriple$extension"
    Copy-Item -Force -LiteralPath $source -Destination $destination
}
