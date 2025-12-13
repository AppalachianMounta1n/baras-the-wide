alias r:= run-debug
alias p50:= parse-50mb

run-debug:
  ./target/debug/baras


parse-50mb:
  ./target/debug/baras parse-file --path './test-log-files/50mb/combat_2025-12-10_18_12_15_087604.txt'
