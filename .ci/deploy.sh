#!/bin/bash

set -eo pipefail

dput -f deb/*.changes
ssh -l "$REPO_USER" "$REPO_HOST" reprepro -Vb "$REPO_PATH" processincoming default
