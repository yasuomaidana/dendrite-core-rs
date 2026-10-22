#!/usr/bin/env bash
# Fails when a tracked code file has more than MAX_LINES lines (default 500), tests included.
# Markdown, data and generated files are exempt.
set -euo pipefail
max="${MAX_LINES:-500}"
status=0
while IFS= read -r f; do
  [ -f "$f" ] || continue
  n=$(wc -l < "$f" | tr -d ' ')
  if [ "$n" -gt "$max" ]; then
    echo "$f: $n lines (max $max)"
    status=1
  fi
done < <(git ls-files '*.go' '*.rs' '*.py' '*.ts' '*.tsx' '*.js' '*.sh' ':!:**/testdata/**' ':!:**/vendor/**' ':!:**/*.pb.go' ':!:**/*_generated.go')
exit "$status"
