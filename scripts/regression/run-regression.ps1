param(
  [ValidateSet("smoke", "production-safety", "release", "ui-smoke")]
  [string]$Suite = "production-safety",
  [string]$OutDir = "artifacts/regression",
  [switch]$KeepPassed
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$tauriRoot = Join-Path $repoRoot "src-tauri"
$outPath = Join-Path $repoRoot $OutDir

$argsList = @(
  "run",
  "--bin",
  "regression_runner",
  "--",
  "--suite",
  $Suite,
  "--out-dir",
  $outPath
)

if ($KeepPassed) {
  $argsList += "--keep-passed"
}

Push-Location $tauriRoot
try {
  & cargo @argsList
  exit $LASTEXITCODE
}
finally {
  Pop-Location
}
