#!/bin/bash
#SBATCH --job-name=gpu-matrix
#SBATCH --partition=debuga100
#SBATCH --account=acct-stu
#SBATCH --qos=debug
#SBATCH --nodes=1
#SBATCH --ntasks-per-node=1
#SBATCH --cpus-per-task=2
#SBATCH --gres=gpu:nvidia_a100_1g.5gb:1
#SBATCH --time=00:05:00
#SBATCH --output=%j.out
#SBATCH --error=%j.err

set -euo pipefail

bash build.sh
./matrix_ops 512 20
