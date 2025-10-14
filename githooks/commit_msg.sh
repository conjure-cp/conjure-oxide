#!/bin/sh

commit_msg_file="$1"
commit_msg=$(cat "$commit_msg_file")

# Allow merge commits
if echo "$commit_msg" | grep -qE "^Merge "; then
  exit 0
fi

# Regex for the required format (scope is optional)
valid_format_regex="^(feat|fix|docs|style|refactor|test|chore)(\([^)]+\))?: .+"

# Regex to check if a scope is present
scope_present_regex="^(feat|fix|docs|style|refactor|test|chore)\([^)]+\): .+"

# First, validate the overall format
if ! echo "$commit_msg" | grep -qE "$valid_format_regex"; then
  echo "Error: Invalid commit message format." >&2
  echo "Please use the format: <type>(<optional scope>): <description>" >&2
  echo "Example: feat(auth): add login endpoint" >&2
  exit 1
fi

# If the format is valid, check for the scope to give a warning
if ! echo "$commit_msg" | grep -qE "$scope_present_regex"; then
  echo "Warning: Commit message is missing a scope." >&2
  echo "Consider adding a scope like: <type>(scope): <description>" >&2
fi

exit 0
