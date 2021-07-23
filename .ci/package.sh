#!/bin/bash

set -eo pipefail

./changelog.sh

DIR=deb/gbp

git worktree add ${DIR} HEAD
rm ${DIR}/.git
git worktree prune

mv {,${DIR}/}debian/changelog

pushd ${DIR}
dpkg-buildpackage
popd
rm -rf ${DIR}
