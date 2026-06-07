# CUDA Attention Demo

This example implements the core computation used by Transformer attention:

```text
Attention(Q, K, V) = softmax(QK^T / sqrt(head_dim)) V
```

CUDA kernels:

- `qk_tiled_kernel`: shared-memory tiled matrix multiplication for `QK^T`.
- `softmax_rows_kernel`: one CUDA block reduces and normalizes one attention row.
- `av_tiled_kernel`: shared-memory tiled multiplication for `Attention * V`.
- `fill_qkv`: initializes deterministic input tensors on GPU.

Default Slurm run:

```bash
sbatch job.slurm
```

Default program parameters:

```text
./attention_demo 2 4 256 64 10
```

Meaning:

```text
batches=2
heads=4
seq_len=256
head_dim=64
repeats=10
```

Expected output includes device name, effective GFLOPS, checksum, and CPU reference error for one sampled output.
