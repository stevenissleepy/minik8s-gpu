# Minik8s GPU

这个插件通过 Minik8s CRD 把 CUDA 任务提交到远程 Slurm 集群执行

- Controller 层：`gpu-plugin-controller` watch `GPUJob`，为每个任务创建独立的 runner Pod，并持续维护任务状态。
- Runner 层：`gpujob-runner` 读取 `GPUJob` 和 `HPCCredential`，通过 SSH 上传源码，使用 `sbatch` 提交任务，轮询 `squeue` / `sacct`，最后把状态和输出写回 Minik8s。


## Install

在一个可以使用 `kubectl` 连接到 Minik8s 集群的 Linux 机器上

首先从 GitHub Release 安装 GPUJob 结果查询工具：

```sh
tag=v0.1.0
deb="minik8s-gpu-${tag}-linux-all.deb"
curl -fLO "https://github.com/stevenissleepy/minik8s-gpu/releases/download/${tag}/${deb}"
sudo apt install "./${deb}"
```

然后下载部署清单，注册 GPU CRD，并启动 `gpu-plugin-controller`：

```sh
tag=v0.1.0
base="https://raw.githubusercontent.com/stevenissleepy/minik8s-gpu/${tag}/deploy"
curl -fLO "${base}/gpu-crds.yaml"
curl -fLO "${base}/gpu-plugin-controller.yaml"
kubectl apply -f gpu-crds.yaml
kubectl apply -f gpu-plugin-controller.yaml
```

`gpu-plugin-controller.yaml` 使用 `ghcr.io/stevenissleepy/gpu-plugin-controller:latest` 启动 controller，并使用 `ghcr.io/stevenissleepy/gpujob-runner:latest` 创建 runner Pod。


## Usage

### 创建 HPC 凭据

创建 runner 登录远程 Slurm 集群时使用的凭据：

```sh
cat >hpccredential.yaml <<'EOF'
apiVersion: gpu.minik8s.io/v1alpha1
kind: HPCCredential
metadata:
  name: sjtu-hpc
spec:
  username: <your-hpc-username>
  password: "<your-hpc-password>"
  knownHosts: ""
EOF

kubectl apply -f hpccredential.yaml
```

`HPCCredential` 支持 `password` 或 `privateKey`。当前实现会把凭据直接保存在 custom resource 中，只适合实验环境；不要把真实凭据提交到 Git，使用完成后应立即删除。

### 提交 GPU 任务

下载矩阵运算示例，并根据实际环境修改 `credentialRef`、`hpc.loginHost` 和 `slurm` 配置：

```sh
tag=v0.1.0
url="https://raw.githubusercontent.com/stevenissleepy/minik8s-gpu/${tag}/examples/matrix-demo/gpujob-matrix.yaml"
curl -fLo gpujob-matrix.yaml "$url"
kubectl apply -f gpujob-matrix.yaml
```

`GPUJob` 的主要配置包括：

| 字段 | 含义 |
|------|------|
| `spec.credentialRef` | 引用同 namespace 下的 `HPCCredential` |
| `spec.hpc.loginHost` | HPC 登录节点地址 |
| `spec.slurm` | partition、account、qos、GPU、时间和输出文件等 Slurm 参数 |
| `spec.source.files` | 需要上传的源码和构建脚本 |
| `spec.run.command` | Slurm 任务最终执行的命令 |
| `spec.run.collect` | 任务结束后需要收集的输出文件 |

任务执行流程为：

```text
GPUJob
  -> gpu-plugin-controller 创建 runner Pod
  -> gpujob-runner 上传源码并生成 Slurm 脚本
  -> sbatch 提交任务，squeue / sacct 轮询状态
  -> 任务状态写回 GPUJob，输出文件写入结果 ConfigMap
```


## 查看状态和结果

```sh
kubectl get gpujobs -A
kubectl get gpujobs gpu-matrix -o yaml
kubectl get pods -l gpujob.minik8s.io/name=gpu-matrix -o wide
```

任务完成后，使用安装的结果查询工具读取 GPUJob 状态和结果 ConfigMap：

```sh
kubectl-gpu-result.sh gpu-matrix
kubectl-gpu-result.sh gpu-matrix --all
kubectl-gpu-result.sh gpu-matrix -o json
```

如果任务长时间处于 `PENDING`，可以从 `GPUJob.status` 中查看 `slurmJobId`、`slurmState`、`queryCommand` 和 `message`。`AssocGrpGRES` 表示 Slurm 账号或 association 的 GPU GRES 配额不足，并非 Minik8s 上传或状态回写失败。


## 清理

```sh
kubectl delete gpujobs gpu-matrix
kubectl delete hpccredentials sjtu-hpc
rm -f hpccredential.yaml gpujob-matrix.yaml
```


## 完整测试

完整的联调和故障排查方法参见 [test.md](docs/test.md)。
