#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

image_ref="${IMAGE_REF:-ghcr.io/stevenissleepy/gpu-plugin-controller:latest}"

cargo build --release -p gpu-plugin-controller
docker build -t "${image_ref}" -f- . <<'DOCKERFILE'
FROM ubuntu:24.04
COPY target/release/gpu-plugin-controller /usr/local/bin/gpu-plugin-controller
DOCKERFILE
docker push "${image_ref}"

echo "built and pushed image ${image_ref}"
