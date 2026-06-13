#!/bin/bash
#SBATCH --job-name=minik8s-attention-demo
#SBATCH --partition=debuga100
#SBATCH --account=acct-stu
#SBATCH --qos=debug
#SBATCH --nodes=1
#SBATCH --ntasks-per-node=1
#SBATCH --cpus-per-task=2
#SBATCH --gres=gpu:1
#SBATCH --time=00:10:00
#SBATCH --output=%j.out
#SBATCH --error=%j.err

set -eo pipefail

bash build.sh
./attention_demo 2 4 256 64 10
