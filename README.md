# Minik8s GPU Plugin

GPU 插件通过 CRD 提供 `GPUJob` 和 `HPCCredential`，由 `gpu-plugin-controller` 为每个 `GPUJob` 创建独立 runner Pod。runner 会把源码上传到交我算，执行 `sbatch`，轮询 `squeue` / `sacct`，并把状态和结果写回 Minik8s。

## 目录结构

- `crates/api`：`GPUJob`、`HPCCredential` 的 typed API schema。
- `crates/controller`：watch `GPUJob`，为每个任务创建 runner Pod。
- `crates/runner`：运行在 runner Pod 中，负责上传源码、提交 Slurm 任务、收集结果和回写状态。
- `deploy/gpu-crds.yaml`：注册 GPU 插件 CRD。
- `deploy/gpu-plugin-controller.yaml`：启动 GPU controller。
- `examples/attention-demo`：CUDA attention 验收示例。
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
minik8s/gpu-plugin-controller:latest
minik8s/gpujob-runner:latest
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

`deploy/gpu-plugin-controller.yaml` 默认把 controller 固定到 control-plane，并使用 `minik8s/gpujob-runner:latest` 创建 runner Pod。

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

## 提交 Demo

```sh
kubectl apply -f crates/plugin/gpu/examples/attention-demo/gpujob-attention.yaml
kubectl get gpujobs -A
kubectl get pods -l gpujob.minik8s.io/name=gpu-attention -o wide
```

查看状态和结果：

```sh
export MINIK8S_APISERVER='http://127.0.0.1:8080'
curl -s "$MINIK8S_APISERVER/apis/gpu.minik8s.io/v1alpha1/namespaces/default/gpujobs/gpu-attention" | jq .status
kubectl get configmap gpujob-gpu-attention-result
curl -s "$MINIK8S_APISERVER/api/v1/namespaces/default/configmaps/gpujob-gpu-attention-result" | jq .data
```

本地只验证 runner 生成 Slurm 脚本时，可以直接运行 dry-run。dry-run 仍会读取 API Server 中的 `GPUJob` 和 `HPCCredential`，但不会执行 SSH/SCP/sbatch：

```sh
MINIK8S_APISERVER=http://127.0.0.1:8080 \
cargo run -p gpujob-runner -- \
  --api-server=http://127.0.0.1:8080 \
  --namespace=default \
  --name=gpu-attention \
  --dry-run
```

如果 `GPUJob.status.slurmState` 长时间是 `PENDING`，并且 `message` 中出现 `AssocGrpGRES`，说明任务已经提交到交我算，但被 Slurm 账号或 association 的 GPU GRES 配额限制挡住；这不是 Minik8s 上传或状态回写失败。
