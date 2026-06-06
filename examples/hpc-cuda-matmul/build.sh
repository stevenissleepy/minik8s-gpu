#!/bin/bash
set -euo pipefail

source /etc/profile
module load cuda/12.2.2

nvcc -O3 -arch=sm_80 main.cu -o matmul_bench
