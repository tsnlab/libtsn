#!/bin/bash

set -eo pipefail

dput -f deb/*.changes
ssh packages reprepro -Vb "$REPO_PATH" processincoming default
