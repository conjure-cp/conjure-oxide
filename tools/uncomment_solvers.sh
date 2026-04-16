#!/bin/bash

# Usage: ./uncomment_solvers.sh <keyword> <solver>
# Example: ./uncomment_solvers.sh "div" "sat-order"

[[ $# -ne 2 ]] && { echo "Usage: $0 <keyword> <solver>"; exit 1; }

KEYWORD=$1
SOLVER=$2

is_match() {
    local dir=$1
    local dir_name="${dir##*/}"

    if [[ "$KEYWORD" == "/" ]]; then
        grep -Pq '(?<!\\)/(?!\\)' "$dir"/*.essence "$dir"/*.eprime 2>/dev/null
    else
        grep -Fq "$KEYWORD" "$dir"/*.essence "$dir"/*.eprime 2>/dev/null
    fi
}

while IFS= read -r config; do
    dir="${config%/*}" 
    
    if is_match "$dir" && grep -q "# \"$SOLVER\"" "$config"; then
        echo "Uncommenting $SOLVER in $config"
        sed -i "s|# \"$SOLVER\"|\"$SOLVER\"|" "$config"
    fi
done < <(find tests-integration/tests/integration -name "config.toml")