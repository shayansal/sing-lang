$ErrorActionPreference = "Stop"
cargo build --release -p sing_cli
Write-Output "built target\release\sing.exe"
