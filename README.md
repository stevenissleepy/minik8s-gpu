# Minik8s GPU Plugin

GPU 插件通过 CRD 提供 `GPUJob` 和 `HPCCredential`，由 `gpu-plugin-controller` 为每个 `GPUJob` 创建独立 runner Pod。runner 会把源码上传到交我算，执行 `sbatch`，轮询 `squeue` / `sacct`，并把状态和结果写回 Minik8s。

## 目录结构

- `crates/api`：`GPUJob`、`HPCCredential` 的 typed API schema。
- `crates/controller`：watch `GPUJob`，为每个任务创建 runner Pod。
- `crates/runner`：运行在 runner Pod 中，负责上传源码、提交 Slurm 任务、收集结果和回写状态。
- `deploy/gpu-crds.yaml`：注册 GPU 插件 CRD。
- `deploy/gpu-plugin-controller.yaml`：启动 GPU controller。
- `examples/matrix-demo`：CUDA 矩阵加法和矩阵乘法验收示例。
- `examples/attention-demo`：CUDA attention 扩展示例。
- `scripts`：GPU 插件镜像构建脚本。

## 构建

```sh
cargo check -p gpu-plugin-api
cargo check -p gpu-plugin-controller
cargo check -p gpujob-runner
```

构建并推送默认镜像：

```sh
bash crates/plugin/gpu/scripts/build-controller-image.sh
bash crates/plugin/gpu/scripts/build-runner-image.sh
```

默认镜像名：

```text
stevenissleepy/gpu-plugin-controller:latest
stevenissleepy/gpujob-runner:latest
```

可以用 `IMAGE_REF` 覆盖单个脚本的目标镜像。

## 安装

先构建并加载或推送 `gpu-plugin-controller` 和 `gpujob-runner` 镜像，然后安装 CRD 和 controller：

```sh
kubectl apply -f crates/plugin/gpu/deploy/gpu-crds.yaml
kubectl apply -f crates/plugin/gpu/deploy/gpu-plugin-controller.yaml
kubectl get crds
kubectl get pods -n kube-system -o wide
```

`deploy/gpu-plugin-controller.yaml` 默认把 controller 固定到 control-plane，并使用 `stevenissleepy/gpujob-runner:latest` 创建 runner Pod。

## 凭据

创建交我算密码凭据。不要把真实密码提交到 Git；测试结束后删除这个资源：

```sh
cat >/tmp/hpccred-sjtu.yaml <<'EOF'
apiVersion: gpu.minik8s.io/v1alpha1
kind: HPCCredential
metadata:
  name: sjtu-hpc
spec:
  username: <your-hpc-username>
  password: "<your-hpc-password>"
  knownHosts: ""
EOF

kubectl apply -f /tmp/hpccred-sjtu.yaml
kubectl get hpccredentials
```

`HPCCredential` 当前是课程项目简化实现，私钥或密码以 custom resource 保存；真实系统应替换为 `Secret` 或等价加密存储。使用 `password` 模式时，runner 镜像内需要 `sshpass`，默认 runner 镜像已经安装。

## 提交矩阵 Demo

```sh
kubectl apply -f crates/plugin/gpu/examples/matrix-demo/gpujob-matrix.yaml
kubectl get gpujobs -A
kubectl get pods -l gpujob.minik8s.io/name=gpu-matrix -o wide
```

`gpujob-matrix.yaml` 的顶层字段包含 `apiVersion`、`kind: GPUJob`、`metadata.name: gpu-matrix` 和 `spec`。`spec.slurm` 包含 Slurm 脚本需要的 partition、account、qos、nodes、ntasksPerNode、cpusPerTask、gres、time、output 和 error；`spec.source.files` 内嵌 CUDA 源码和构建脚本；`spec.run.command` 是最终执行命令。

查看状态和结果：

```sh
kubectl get gpujobs -A
bash crates/plugin/gpu/scripts/kubectl-gpu-result gpu-matrix
bash crates/plugin/gpu/scripts/kubectl-gpu-result gpu-matrix --all
bash crates/plugin/gpu/scripts/kubectl-gpu-result gpu-matrix -o json
```

CUDA 程序代码路径：

```text
crates/plugin/gpu/examples/matrix-demo/matrix_ops.cu
```

并发方式：`matrix_add_kernel` 用一维 grid 把每个矩阵元素分配给一个 CUDA thread；`matrix_mul_kernel` 用二维 thread block 计算输出矩阵 tile，并用 shared memory 缓存输入 tile，让多个 block/warp 并发计算不同输出区域。

本地只验证 runner 生成 Slurm 脚本时，可以直接运行 dry-run。dry-run 仍会读取 API Server 中的 `GPUJob` 和 `HPCCredential`，但不会执行 SSH/SCP/sbatch：

```sh
MINIK8S_APISERVER=http://127.0.0.1:8080 \
cargo run -p gpujob-runner -- \
  --api-server=http://127.0.0.1:8080 \
  --namespace=default \
  --name=gpu-matrix \
  --dry-run
```

如果验收环境中任务一直处于 pending，`kubectl get gpujobs -A` 会直接显示 `PHASE=Pending`、`SLURMSTATE=PENDING`、`SLURMJOBID`、`SUBMITTED` 和 `QUERY`。也可以用下面命令查看同一组字段：

```sh
curl -s "$MINIK8S_APISERVER/apis/gpu.minik8s.io/v1alpha1/namespaces/default/gpujobs/gpu-matrix" \
  | jq '.status | {phase, slurmState, slurmJobId, submittedTime, queryCommand, message}'
```

如果 `message` 中出现 `AssocGrpGRES`，说明任务已经提交到交我算，但被 Slurm 账号或 association 的 GPU GRES 配额限制挡住；这不是 Minik8s 上传或状态回写失败。

## GPU 任务命令清单

以下命令覆盖本任务验收时需要展示的完整流程。默认 API Server 地址为本地 `127.0.0.1:8080`，GPUJob 名称为 `gpu-matrix`，namespace 为 `default`。

### 1. 准备环境变量

```sh
export MINIK8S_APISERVER='http://127.0.0.1:8080'
```

### 2. 安装 GPU 插件

```sh
kubectl apply -f crates/plugin/gpu/deploy/gpu-crds.yaml
kubectl apply -f crates/plugin/gpu/deploy/gpu-plugin-controller.yaml
kubectl get crds
kubectl get pods -n kube-system -o wide
```

### 3. 创建 HPC 凭据

```sh
cat >/tmp/hpccred-sjtu.yaml <<'EOF'
apiVersion: gpu.minik8s.io/v1alpha1
kind: HPCCredential
metadata:
  name: sjtu-hpc
spec:
  username: <your-hpc-username>
  password: "<your-hpc-password>"
  knownHosts: ""
EOF

kubectl apply -f /tmp/hpccred-sjtu.yaml
kubectl get hpccredentials
```

### 4. 提交 GPU 任务

```sh
kubectl apply -f crates/plugin/gpu/examples/matrix-demo/gpujob-matrix.yaml
```

### 5. 获取任务提交情况

```sh
kubectl get gpujobs
kubectl get gpujobs -A
kubectl get gpujobs gpu-matrix -o yaml
```

表格输出会包含 `PHASE`、`SLURMSTATE`、`SLURMJOBID`、`SUBMITTED`、`QUERY` 和 `AGE`。其中 `SLURMJOBID` 是 Slurm 任务 ID，`QUERY` 是登录 HPC 后可直接使用的查询命令。

### 6. 获取详细状态

```sh
curl -s "$MINIK8S_APISERVER/apis/gpu.minik8s.io/v1alpha1/namespaces/default/gpujobs/gpu-matrix" \
  | jq .status
```

只看验收关键字段：

```sh
curl -s "$MINIK8S_APISERVER/apis/gpu.minik8s.io/v1alpha1/namespaces/default/gpujobs/gpu-matrix" \
  | jq '.status | {phase, reason, message, runnerPod, slurmJobId, submittedTime, queryCommand, slurmState, stdoutTail, stderrTail, resultConfigMap}'
```

### 7. 获取 runner Pod 情况

```sh
kubectl get pods -l gpujob.minik8s.io/name=gpu-matrix -o wide
kubectl get pods -l component=gpujob-runner -A
```

### 8. 获取 GPU 任务返回结果

任务完成后，runner 会把 Slurm 输出文件写入结果 ConfigMap：

```sh
bash crates/plugin/gpu/scripts/kubectl-gpu-result gpu-matrix
bash crates/plugin/gpu/scripts/kubectl-gpu-result gpu-matrix --all
bash crates/plugin/gpu/scripts/kubectl-gpu-result gpu-matrix -o json
```

只看 stdout/stderr：

```sh
bash crates/plugin/gpu/scripts/kubectl-gpu-result gpu-matrix
```

实际 ConfigMap key 会按收集到的文件名生成，默认是 `<slurmJobId>.out` 和 `<slurmJobId>.err`；GPU 插件结果查询脚本会自动读取 GPUJob status 中的 `resultConfigMap` 并提取对应输出。

### 9. pending 状态排查

如果验收环境中任务一直处于 pending，先输出 Minik8s 侧状态、Slurm 任务 ID、提交时间和查询命令：

```sh
kubectl get gpujobs -A
curl -s "$MINIK8S_APISERVER/apis/gpu.minik8s.io/v1alpha1/namespaces/default/gpujobs/gpu-matrix" \
  | jq '.status | {phase, slurmState, slurmJobId, submittedTime, queryCommand, message}'
```

如果 `queryCommand` 为 `squeue -j <job-id>`，可以登录 HPC 后执行：

```sh
squeue -j <job-id>
sacct -n -X -j <job-id> -P --format=JobID,State,ExitCode,Elapsed,MaxRSS,ReqMem
```

### 10. 本地 dry-run 生成 Slurm 脚本

```sh
MINIK8S_APISERVER=http://127.0.0.1:8080 \
cargo run -p gpujob-runner -- \
  --api-server=http://127.0.0.1:8080 \
  --namespace=default \
  --name=gpu-matrix \
  --dry-run
```

### 11. 清理资源

```sh
curl -s -X DELETE "$MINIK8S_APISERVER/apis/gpu.minik8s.io/v1alpha1/namespaces/default/gpujobs/gpu-matrix"
curl -s -X DELETE "$MINIK8S_APISERVER/apis/gpu.minik8s.io/v1alpha1/namespaces/default/hpccredentials/sjtu-hpc"
rm -f /tmp/hpccred-sjtu.yaml
```
