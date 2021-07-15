#!/bin/bash

set -eo pipefail

export DEBEMAIL='TSNLab <cto@tsnlab.com>'

if ! version=$(git describe --tags | sed 's/^v//'); then
    version='0'
fi

echo "version: $version"

if prev=$(git describe --tags --match 'v*' HEAD^ --abbrev=0 2>/dev/null); then
    since="--since=$prev"
else
    since=''
fi

rm debian/changelog || true

gbp dch -D unstable -R ${since} --ignore-branch --spawn-editor=never
sed "1s/\(unknown\)/${version}/" -i debian/changelog

DIR=deb/gbp

git worktree add ${DIR}
rm ${DIR}/.git
git worktree prune

mv {,${DIR}/}debian/changelog

pushd ${DIR}
debuild
popd
rm -rf ${DIR}
