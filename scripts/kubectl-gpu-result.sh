#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  cat <<EOF
Usage:
  kubectl-gpu-result.sh <gpujob-name> [-n namespace] [--all] [-o json]

Examples:
  kubectl-gpu-result.sh gpu-matrix
  kubectl-gpu-result.sh gpu-matrix --all
  kubectl-gpu-result.sh gpu-matrix -o json
EOF
}

name=""
namespace="default"
show_all=0
output=""

while (($# > 0)); do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    -n|--namespace)
      if (($# < 2)); then
        echo "error: $1 requires a namespace" >&2
        exit 2
      fi
      namespace="$2"
      shift 2
      ;;
    --all)
      show_all=1
      shift
      ;;
    -o|--output)
      if (($# < 2)); then
        echo "error: $1 requires an output format" >&2
        exit 2
      fi
      output="$2"
      shift 2
      ;;
    -*)
      echo "error: unknown argument $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ -n "$name" ]]; then
        echo "error: only one GPUJob name is supported" >&2
        exit 2
      fi
      name="$1"
      shift
      ;;
  esac
done

if [[ -z "$name" ]]; then
  usage >&2
  exit 2
fi

if [[ -n "$output" && "$output" != "json" ]]; then
  echo "error: unsupported output format ${output}; supported formats are json" >&2
  exit 2
fi

gpujob_json="$(kubectl get gpujobs "$name" -n "$namespace" -o json)"
status_json="$(jq -c '.status // {}' <<<"$gpujob_json")"
phase="$(jq -r '.phase // ""' <<<"$status_json")"
message="$(jq -r '.message // ""' <<<"$status_json")"
runner_pod="$(jq -r '.runnerPod // ""' <<<"$status_json")"
slurm_job_id="$(jq -r '.slurmJobId // ""' <<<"$status_json")"
slurm_state="$(jq -r '.slurmState // ""' <<<"$status_json")"
submitted_time="$(jq -r '.submittedTime // .startTime // .lastProbeTime // ""' <<<"$status_json")"
query_command="$(jq -r '.queryCommand // ""' <<<"$status_json")"
result_configmap="$(jq -r '.resultConfigMap // ""' <<<"$status_json")"
stdout_tail="$(jq -r '.stdoutTail // ""' <<<"$status_json")"
stderr_tail="$(jq -r '.stderrTail // ""' <<<"$status_json")"

configmap_json="{}"
if [[ -n "$result_configmap" ]] && kubectl get configmap "$result_configmap" -n "$namespace" >/dev/null 2>&1; then
  configmap_json="$(kubectl get configmap "$result_configmap" -n "$namespace" -o json)"
fi

stdout_value="$stdout_tail"
stderr_value="$stderr_tail"
if [[ "$configmap_json" != "{}" ]]; then
  stdout_value="$(jq -r --arg job "$slurm_job_id" '
    .data as $data
    | ($data[$job + ".out"] // ($data | to_entries[]? | select(.key | endswith(".out")) | .value) // "")
  ' <<<"$configmap_json")"
  stderr_value="$(jq -r --arg job "$slurm_job_id" '
    .data as $data
    | ($data[$job + ".err"] // ($data | to_entries[]? | select(.key | endswith(".err")) | .value) // "")
  ' <<<"$configmap_json")"
fi

if [[ "$output" == "json" ]]; then
  jq -n \
    --arg name "$name" \
    --arg namespace "$namespace" \
    --arg phase "$phase" \
    --arg message "$message" \
    --arg runnerPod "$runner_pod" \
    --arg slurmJobId "$slurm_job_id" \
    --arg slurmState "$slurm_state" \
    --arg submittedTime "$submitted_time" \
    --arg queryCommand "$query_command" \
    --arg resultConfigMap "$result_configmap" \
    --arg stdout "$stdout_value" \
    --arg stderr "$stderr_value" \
    --argjson data "$(jq -c '.data // {}' <<<"$configmap_json")" \
    '{
      name: $name,
      namespace: $namespace,
      phase: $phase,
      message: $message,
      runnerPod: $runnerPod,
      slurmJobId: $slurmJobId,
      slurmState: $slurmState,
      submittedTime: $submittedTime,
      queryCommand: $queryCommand,
      resultConfigMap: $resultConfigMap,
      stdout: $stdout,
      stderr: $stderr,
      files: $data
    }'
  exit 0
fi

printf 'name: %s\n' "$name"
printf 'namespace: %s\n' "$namespace"
printf 'phase: %s\n' "${phase:-"-"}"
printf 'slurmJobId: %s\n' "${slurm_job_id:-"-"}"
printf 'slurmState: %s\n' "${slurm_state:-"-"}"
printf 'submittedTime: %s\n' "${submitted_time:-"-"}"
printf 'queryCommand: %s\n' "${query_command:-"-"}"
printf 'resultConfigMap: %s\n' "${result_configmap:-"-"}"
if [[ -n "$message" ]]; then
  printf 'message: %s\n' "$message"
fi

if ((show_all)) && [[ "$configmap_json" != "{}" ]]; then
  jq -r '
    (.data // {})
    | to_entries[]
    | "\n==> \(.key) <==\n\(.value)"
  ' <<<"$configmap_json"
  exit 0
fi

if [[ -n "$stdout_value" ]]; then
  printf '\n%s' "$stdout_value"
  [[ "$stdout_value" == *$'\n' ]] || printf '\n'
fi
if [[ -n "$stderr_value" ]]; then
  printf '\n[stderr]\n%s' "$stderr_value" >&2
  [[ "$stderr_value" == *$'\n' ]] || printf '\n' >&2
fi
if [[ -z "$stdout_value" && -z "$stderr_value" ]]; then
  printf '\nGPUJob result is not available yet.\n'
fi
