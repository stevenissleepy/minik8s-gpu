# GPU 插件联调流程

## 前置条件

- Minik8s API Server 已启动，`kubectl` 能访问 control-plane。
- `minik8s/gpu-plugin-controller:latest` 和 `minik8s/gpujob-runner:latest` 已经构建并能被集群拉取。
- 已准备交我算账号、密码或私钥，以及对应 Slurm 分区和账号配置。

## 安装插件

```sh
kubectl apply -f crates/plugin/gpu/deploy/gpu-crds.yaml
kubectl apply -f crates/plugin/gpu/deploy/gpu-plugin-controller.yaml
kubectl get crds
kubectl get pods -n kube-system -o wide
```

安装后 discovery 应出现：

```text
gpujobs.gpu.minik8s.io
hpccredentials.gpu.minik8s.io
```

## 创建 HPC 凭据

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

## 提交 CUDA Attention 示例

```sh
kubectl apply -f crates/plugin/gpu/examples/attention-demo/gpujob-attention.yaml
kubectl get gpujobs -A
kubectl get pods -l gpujob.minik8s.io/name=gpu-attention -o wide
```

关注 `GPUJob.status`：

```sh
export MINIK8S_APISERVER='http://127.0.0.1:8080'
curl -s "$MINIK8S_APISERVER/apis/gpu.minik8s.io/v1alpha1/namespaces/default/gpujobs/gpu-attention" | jq .status
```

关键字段：

```yaml
status:
  phase:
  reason:
  message:
  runnerPod:
  slurmJobId:
  slurmState:
  exitCode:
  stdoutTail:
  stderrTail:
  resultConfigMap:
```

任务完成后，如果 runner 拉回了输出文件，会创建结果 ConfigMap：

```sh
kubectl get configmap gpujob-gpu-attention-result
curl -s "$MINIK8S_APISERVER/api/v1/namespaces/default/configmaps/gpujob-gpu-attention-result" | jq .data
```

## 清理

```sh
curl -s -X DELETE "$MINIK8S_APISERVER/apis/gpu.minik8s.io/v1alpha1/namespaces/default/gpujobs/gpu-attention"
curl -s -X DELETE "$MINIK8S_APISERVER/apis/gpu.minik8s.io/v1alpha1/namespaces/default/hpccredentials/sjtu-hpc"
rm -f /tmp/hpccred-sjtu.yaml
```

## 常见失败

| 失败来源 | `status.phase` | `status.reason` |
|----------|----------------|-----------------|
| SSH/SCP 失败 | `Failed` | `UploadFailed` / `LoginFailed` / `RunnerFailed` |
| `sbatch` 失败 | `Failed` | `SubmitFailed` / `RunnerFailed` |
| 编译失败 | `Failed` | `RuntimeFailed` 或 `CompileFailed` |
| Slurm OOM | `Failed` | `OutOfMemory` |
| Slurm 超时 | `Failed` | `TimeLimitExceeded` |
| Slurm 取消 | `Cancelled` | `Cancelled` |

如果 `GPUJob.status.slurmState` 长时间是 `PENDING`，并且 `message` 中出现 `AssocGrpGRES`，说明任务已经提交到交我算，但被 Slurm 账号或 association 的 GPU GRES 配额限制挡住；这不是 Minik8s 上传或状态回写失败。
