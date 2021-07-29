#!/usr/bin/env bash

set -eo pipefail

if [ -z "$CI" ]; then
    exit 0
fi

mkdir -p ~/.ssh/
echo "$SSH_KEY" > ~/.ssh/id_rsa
chmod 600 ~/.ssh/id_rsa

cat << EOF > ~/.ssh/config
Host packages
    Hostname $REPO_HOST
    Port $REPO_PORT
    User $REPO_USER
EOF

ssh-keyscan -p "$REPO_PORT" "$REPO_HOST" >> ~/.ssh/known_hosts
