# HPC CUDA Matrix Multiplication Example

Files:

- `main.cu`: shared-memory tiled matrix multiplication plus matrix addition.
- `build.sh`: loads CUDA on SJTU HPC and compiles the CUDA program.
- `gpujob-matmul.yaml`: ready-to-submit `GPUJob` with inline source files.
- `hpccredential.template.yaml`: credential template. Fill the password outside git.

Submit order:

```bash
kubectl apply -f crates/plugin/gpu/crds/gpujobs.yaml
kubectl apply -f crates/plugin/gpu/crds/hpccredentials.yaml
kubectl apply -f hpccredential.yaml
kubectl apply -f gpujob-matmul.yaml
gpujob-runner --api-server=http://127.0.0.1:8080 --namespace=default --name=cuda-matmul --poll-seconds=10
```
