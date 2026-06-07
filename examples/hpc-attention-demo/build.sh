#!/bin/bash
set -eo pipefail

source /etc/profile
module load cuda/12.2.2

set -u

nvcc -O3 -arch=sm_80 attention_demo.cu -o attention_demo
