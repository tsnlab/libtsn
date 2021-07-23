#!/bin/bash

set -eo pipefail

export DEBEMAIL='TSN Lab, Inc. <cto@tsnlab.com>'

if ! version=$(git describe --tags | sed 's/^v//'); then
    version='0'
    # XXX: testing for ci
    version='0.0.0+testing'
fi

echo "version: $version"

if prev=$(git describe --tags --match 'v*' HEAD^ --abbrev=0 2>/dev/null); then
    since="--since=$prev"
fi

rm debian/changelog || true

if [ -n "$since" ]; then
    gbp dch -D unstable -R "${since}" --ignore-branch --spawn-editor=never
else
    gbp dch -D unstable -R --ignore-branch --spawn-editor=never
fi
sed "1s/\(unknown\)/${version}/" -i debian/changelog
