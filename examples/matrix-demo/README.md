# GPU Matrix Demo

This example submits a CUDA matrix workload through a `GPUJob` YAML.

Files:

- `gpujob-matrix.yaml`: Minik8s GPUJob manifest. It contains `kind: GPUJob` and `metadata.name: gpu-matrix`, plus the Slurm partition, account, QoS, node, CPU, GRES, time limit, output, and error settings.
- `matrix_ops.cu`: CUDA matrix addition and tiled matrix multiplication kernels.
- `build.sh`: compiles the CUDA program on SJTU HPC.
- `job.slurm`: equivalent standalone Slurm script for checking the generated settings.

Submit through Minik8s:

```bash
kubectl apply -f crates/plugin/gpu/examples/matrix-demo/gpujob-matrix.yaml
kubectl get gpujobs -A
```

Get returned output:

```bash
bash crates/plugin/gpu/scripts/kubectl-gpu-result gpu-matrix
bash crates/plugin/gpu/scripts/kubectl-gpu-result gpu-matrix --all
bash crates/plugin/gpu/scripts/kubectl-gpu-result gpu-matrix -o json
```

The CUDA program uses GPU concurrency by launching many thread blocks:

- `matrix_add_kernel` maps one CUDA thread to one matrix element, so additions across the whole matrix run in parallel.
- `matrix_mul_kernel` maps a 2D grid of thread blocks to output tiles and uses shared-memory tiles to reuse input values while many output elements are computed concurrently.
