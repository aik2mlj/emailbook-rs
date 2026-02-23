#!/bin/sh
projectdir="$(cd -P -- "$(dirname -- "$(dirname -- "$0")")" && pwd -P)"
source "$projectdir/test/utils.sh"

book=/tmp/book.txt
rm $book 2> /dev/null
cat "$projectdir/test/sample1" \
    | "$projectdir/emailbook-hare" "$book" --parse --all
cat "$projectdir/test/sample2" \
    | "$projectdir/emailbook-hare" "$book" --parse --all
diff -s "$book" "$projectdir/test/book-expected.txt" \
    && success \
    || failure && exit 1
