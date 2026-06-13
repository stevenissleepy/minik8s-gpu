# GPU Attention Demo

This GPU plugin example demonstrates a CUDA implementation of Transformer attention:

```text
Attention(Q, K, V) = softmax(QK^T / sqrt(head_dim)) V
```

Files:

- `attention_demo.cu`: CUDA kernels for tiled `QK^T`, row-wise softmax, and tiled `Attention * V`.
- `build.sh`: compiles the program on SJTU HPC.
- `job.slurm`: Slurm script verified on `debuga100`.

Run on SJTU HPC:

```bash
sbatch job.slurm
```

Default command:

```bash
./attention_demo 2 4 256 64 10
```

Expected output includes:

```text
CUDA attention demo OK
device=NVIDIA A100-SXM4-40GB MIG 1g.5gb
effective_gflops=...
cpu_abs_error=0.00000000
```

The same source can be embedded into a `GPUJob` YAML by placing `attention_demo.cu` and `build.sh` under `spec.source.files`.
