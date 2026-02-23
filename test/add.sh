#!/bin/sh
projectdir="$(cd -P -- "$(dirname -- "$(dirname -- "$0")")" && pwd -P)"
source "$projectdir/test/utils.sh"

template="$projectdir/test/book-expected.txt"
book="/tmp/book-add.txt"
cp "$template" "$book"
# add Juan
"$projectdir/emailbook-hare" "$book" --add 'Juan Pérez <juan@example.org>'
# skip "John Doe" because it's the same as John Doe (first entry)
"$projectdir/emailbook-hare" "$book" --add '"John Doe" <john.doe@example.com>'

grep -q Juan $book && ! grep -q '"John Doe"' $book \
    && success \
    || failure && exit 1
