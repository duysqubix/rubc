branch := `git rev-parse --abbrev-ref HEAD`
prompt := "Create a detailed commit message from the following changes. Please include a one line summary followed by sections that relate to added,removed, and updated. Ensure to be as detailed as necessary, "

commit-and-push: make-commitmsg push

make-commitmsg:
  git diff --staged | sgpt "{{prompt}}" | tee /tmp/.commitmsg 
  git commit -F /tmp/.commitmsg

push: 
  git push origin {{branch}}
