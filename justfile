branch := `git rev-parse --abbrev-ref HEAD`
prompt := "Create a detailed commit message from the following changes. Please include a one line summary followed by sections that relate to added,removed, and updated. Ensure to be as detailed as necessary, "

commit-and-push: regression-test make-commitmsg push

regression-test: 
  LOG_LEVEL=debug cargo run assets/cpu_instrs/cpu_instrs.gb --panic-on-stuck

make-commitmsg:
  git diff --staged | sgpt "{{prompt}}" | tee /tmp/.commitmsg 
  git commit -F /tmp/.commitmsg

push: 
  git push origin {{branch}}

test-opcodes:
  cargo test --package rubc-core -- --show-output

run args:
  LOG_LEVEL=warn cargo run {{args}}

trun args:
  LOG_LEVEL=debug cargo run {{args}}

trace-run args:
  LOG_LEVEL=debug cargo run {{args}}