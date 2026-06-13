#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/../../../.."

image_ref="${IMAGE_REF:-stevenissleepy/gpujob-runner:latest}"

cargo build --release -p gpujob-runner
docker build -t "${image_ref}" -f- . <<'DOCKERFILE'
FROM ubuntu:24.04
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates openssh-client sshpass \
    && rm -rf /var/lib/apt/lists/*
COPY target/release/gpujob-runner /usr/local/bin/gpujob-runner
DOCKERFILE
docker push "${image_ref}"

echo "built and pushed image ${image_ref}"
