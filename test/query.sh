#!/bin/sh
projectdir="$(cd -P -- "$(dirname -- "$(dirname -- "$0")")" && pwd -P)"
source "$projectdir/test/utils.sh"

book="$projectdir/test/book-expected.txt"
"$projectdir/emailbook-hare" "$book" --search 'mari' > /tmp/emailbook-query
diff -s /tmp/emailbook-query "$projectdir/test/query-expected.txt" \
    && success \
    || failure && exit 1
