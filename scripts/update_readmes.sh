#! /bin/bash
# Updates `../README.md` and `../lints/*/README.md`.

# NOTE: Before commiting, review your changes to those READMEs!

# set -x
set -euo pipefail

if [[ $# -ne 0 ]]; then
    echo "$0: expect no arguments" >&2
    exit 1
fi

SCRIPTS="$(dirname "$(realpath "$0")")"
WORKSPACE="$(realpath "$SCRIPTS"/..)"

cd "$WORKSPACE"/lints

for LIBRARY in *; do
    pushd "$LIBRARY" >/dev/null

    (
        echo "# $LIBRARY"
        echo
        cat src/*.rs |
        sed -n '/^[a-z_:]*_lint! {$/,/^}$/p' |
        gsed -n 's,^[[:space:]]*///\([[:space:]]\(.*\)\)\?$,\2,;T;p'
    ) > README.md

    # prettier --write README.md

    popd >/dev/null
done

TMP="$(mktemp)"

LISTED=

IFS=
cat ../README.md |
while read X; do
    if [[ "$X" =~ ^\| ]]; then
        if [[ -z "$LISTED" ]]; then
            echo '| Library | Description | Anchor | Non Anchor |'
            echo '| - | - | - | - |'
            for DIR in */; do
                CARGO_TOML="${DIR}Cargo.toml"
                README="${DIR}README.md"
                DESC=$(
                    grep -H '^description = "[^"]*"$' "$CARGO_TOML" |
                    sed 's,^\([^/]*\)/Cargo.toml:description = "\([^"]*\)"$,| [`\1`](lints/\1) | \2,'
                )
                ANCHOR=$(
                    grep '^- \[[ x]\] Anchor$' "$README" | cut -d "[" -f2 | cut -d "]" -f1
                )
                NON_ANCHOR=$(
                    grep '^- \[[ x]\] Non Anchor$' "$README" | cut -d "[" -f2 | cut -d "]" -f1
                )
                ANCHOR_COLUMN=" "
                if [[ "$ANCHOR" == "x" ]]; then
                    ANCHOR_COLUMN=":heavy_check_mark:"
                fi
                NON_ANCHOR_COLUMN=" "
                if [[ "$NON_ANCHOR" == "x" ]]; then
                    NON_ANCHOR_COLUMN=":heavy_check_mark:"
                fi
                echo "$DESC | $ANCHOR_COLUMN | $NON_ANCHOR_COLUMN |"
            done
            LISTED=1
        fi
        continue
    fi
    echo "$X"
done |
cat > "$TMP"

mv "$TMP" ../README.md

prettier --write ../README.md
