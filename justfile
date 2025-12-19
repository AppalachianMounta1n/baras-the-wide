alias r:= run-debug
alias p50:= parse-50mb

run-debug:
  ./target/debug/baras

parse-50mb:
  ./target/debug/baras parse-file --path './test-log-files/50mb/combat_2025-12-10_18_12_15_087604.txt'

# Tauri app commands
dev:
  cd app && cargo tauri dev

# Build AppImage/deb (NO_STRIP needed on Arch due to linuxdeploy incompatibility)
bundle:
  cd app && NO_STRIP=1 cargo tauri build

# Build release binary only (no bundle)
build-app:
  cd app && cargo tauri build --no-bundle
