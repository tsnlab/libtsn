#!/usr/bin/env bash

set -eo pipefail

if [ -z "$CI" ]; then
    exit 0
fi

mkdir -p ~/.ssh/
echo "$SSH_KEY" > ~/.ssh/id_rsa
chmod 600 ~/.ssh/id_rsa

ssh-keyscan "$REPO_HOST" >> ~/.ssh/known_hosts

cat << EOF > ~/.dput.cf
[DEFAULT]
default_host_main = tsnlab

[tsnlab]
fqdn = $REPO_HOST
method = scp
login = $REPO_USER
incoming = $REPO_PATH/incoming
ssh_config_options =
    StrictHostKeyChecking no
EOF

echo "$PGP_KEY" | gpg --import
