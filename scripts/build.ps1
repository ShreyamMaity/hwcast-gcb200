# Builds the release assets in dist\ (used locally and by CI):
#   hwcast-gcb200.exe   (the whole app: double-click to install)
#   SHA256SUMS.txt
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
Push-Location $root
try {
  # Keep build-machine paths (user name, cargo cache, checkout folder) out of the release exe.
  $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
  $rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE '.rustup' }
  $env:RUSTFLAGS = "--remap-path-prefix=$cargoHome=cargo --remap-path-prefix=$rustupHome=rustup --remap-path-prefix=$root=hwcast-gcb200 --remap-path-prefix=$env:USERPROFILE=user"
  cargo build --release --locked
  if ($LASTEXITCODE) { throw 'cargo build failed' }

  Remove-Item dist -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force dist | Out-Null
  Copy-Item 'target\release\hwcast-gcb200.exe' dist
  Get-ChildItem dist -File | ForEach-Object { '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name } |
    Set-Content 'dist\SHA256SUMS.txt' -Encoding ascii
  Get-Content 'dist\SHA256SUMS.txt'
} finally {
  Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
  Pop-Location
}
