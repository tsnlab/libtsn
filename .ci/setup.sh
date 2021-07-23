#!/usr/bin/env bash

set -eo pipefail

if [ -z "$CI" ]; then
    exit 0
fi

mkdir -p ~/.ssh/
echo "$SSH_KEY" > ~/.ssh/id_rsa
chmod 600 ~/.ssh/id_rsa

ssh-keyscan "$REPO_HOST" >> ~/.ssh/known_hosts
