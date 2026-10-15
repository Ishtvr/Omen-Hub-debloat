# Compatibility entry point for the replaced GPU-only profile screen.
& (Join-Path $PSScriptRoot 'smoke-presets-ui.ps1')
exit $LASTEXITCODE