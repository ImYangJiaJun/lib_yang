#!/usr/bin/env python3
"""重复采集 YANG 核心运行时 Criterion 数据，并生成非阻断 shadow 汇总。"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import re
import statistics
import subprocess
import sys
import tempfile
import time
import tomllib
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CONFIG_PATH = ROOT / "benchmarks" / "runtime-shadow.toml"
BENCH_SOURCE = ROOT / "crates" / "yang-base" / "benches" / "runtime_baseline.rs"
BENCH_FUNCTION_PATTERN = re.compile(r'\.bench_function\(\s*"([^"]+)"')
METRIC_CLASSES = {"control", "hot_path", "request_path", "startup"}
REQUIRED_CRITERION_FILES = {
    "benchmark.json",
    "estimates.json",
    "sample.json",
    "tukey.json",
}


def sha256(path: Path) -> str:
    """返回文件内容指纹。"""

    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_config(path: Path = CONFIG_PATH) -> dict[str, Any]:
    """加载并验证固定 shadow 工作负载。"""

    config = tomllib.loads(path.read_text(encoding="utf-8"))
    if config.get("schema_version") != 1:
        raise ValueError("runtime shadow 配置仅支持 schema_version = 1")
    if config.get("blocking") is not False:
        raise ValueError("B-01 必须保持 blocking = false")
    if config.get("measurement") != "wall_time_ns":
        raise ValueError("当前采集器只接受 wall_time_ns")
    if (
        isinstance(config.get("repetitions"), bool)
        or not isinstance(config.get("repetitions"), int)
        or config["repetitions"] != 3
    ):
        raise ValueError("B-01 固定执行 3 次外层重复")
    if (
        isinstance(config.get("sample_size"), bool)
        or not isinstance(config.get("sample_size"), int)
        or config["sample_size"] != 50
    ):
        raise ValueError("B-01 固定每轮每项采集 50 个 Criterion 样本")
    warm_up_seconds = finite_number(
        config.get("warm_up_seconds"), "warm_up_seconds"
    )
    measurement_seconds = finite_number(
        config.get("measurement_seconds"), "measurement_seconds"
    )
    if warm_up_seconds <= 0:
        raise ValueError("warm_up_seconds 必须大于 0")
    if measurement_seconds <= 0:
        raise ValueError("measurement_seconds 必须大于 0")
    if config.get("cargo_features") != "default":
        raise ValueError("B-01 固定使用 yang-base 默认 feature 集")

    metrics = config.get("metrics")
    if not isinstance(metrics, list) or not metrics:
        raise ValueError("runtime shadow 必须声明 metrics")
    metric_ids = [metric.get("id") for metric in metrics]
    if any(not isinstance(metric_id, str) or not metric_id for metric_id in metric_ids):
        raise ValueError("每个 metric 都必须有非空 id")
    if len(metric_ids) != len(set(metric_ids)):
        raise ValueError("runtime shadow metric id 不能重复")
    if any(metric.get("class") not in METRIC_CLASSES for metric in metrics):
        raise ValueError(f"metric class 只能是 {sorted(METRIC_CLASSES)}")

    source_ids = BENCH_FUNCTION_PATTERN.findall(BENCH_SOURCE.read_text(encoding="utf-8"))
    if len(source_ids) != len(set(source_ids)):
        raise ValueError("runtime_baseline.rs 存在重复 benchmark id")
    if set(source_ids) != set(metric_ids):
        missing = sorted(set(metric_ids) - set(source_ids))
        unexpected = sorted(set(source_ids) - set(metric_ids))
        raise ValueError(
            f"benchmark 源码与 shadow 契约不一致: missing={missing}, unexpected={unexpected}"
        )
    return config


def percentile(values: list[float], quantile: float) -> float:
    """使用 nearest-rank 计算确定性分位数。"""

    if not values:
        raise ValueError("分位数样本不能为空")
    ordered = sorted(values)
    rank = max(1, math.ceil(quantile * len(ordered)))
    return ordered[rank - 1]


def rounded(value: float) -> float:
    """限制机器输出中的无意义浮点尾数。"""

    return round(value, 6)


def finite_number(value: Any, label: str) -> float:
    """把 JSON 数值规范化为有限浮点数。"""

    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} 必须是数值")
    number = float(value)
    if not math.isfinite(number):
        raise ValueError(f"{label} 必须是有限数值")
    return number


def parse_metric(
    directory: Path, expected_sample_count: int
) -> tuple[dict[str, Any], list[float]]:
    """解析单次 Criterion benchmark 的估计值和原始样本。"""

    missing_files = sorted(
        name for name in REQUIRED_CRITERION_FILES if not (directory / name).is_file()
    )
    if missing_files:
        raise ValueError(f"{directory} 缺少 Criterion 原始文件: {missing_files}")

    benchmark = json.loads((directory / "benchmark.json").read_text(encoding="utf-8"))
    estimates = json.loads((directory / "estimates.json").read_text(encoding="utf-8"))
    sample = json.loads((directory / "sample.json").read_text(encoding="utf-8"))

    estimate_source = "slope" if estimates.get("slope") is not None else "mean"
    estimate = estimates.get(estimate_source)
    if not estimate:
        raise ValueError(f"{directory} 缺少 slope/mean 估计")
    interval = estimate["confidence_interval"]
    iterations = sample.get("iters", [])
    times = sample.get("times", [])
    if not iterations or len(iterations) != len(times):
        raise ValueError(f"{directory} 的 sample iters/times 不完整")
    if len(iterations) != expected_sample_count:
        raise ValueError(
            f"{directory} 样本数应为 {expected_sample_count}，实际为 {len(iterations)}"
        )
    samples_ns = []
    for sample_index, (iterations_count, elapsed_ns) in enumerate(
        zip(iterations, times, strict=True), start=1
    ):
        iterations_count = finite_number(
            iterations_count, f"{directory} sample[{sample_index}].iters"
        )
        elapsed_ns = finite_number(
            elapsed_ns, f"{directory} sample[{sample_index}].times"
        )
        if iterations_count <= 0 or elapsed_ns < 0:
            raise ValueError(f"{directory} 包含非法 Criterion 样本")
        samples_ns.append(elapsed_ns / iterations_count)

    canonical_estimate_ns = finite_number(
        estimate["point_estimate"], f"{directory} canonical estimate"
    )
    if canonical_estimate_ns <= 0:
        raise ValueError(f"{directory} canonical estimate 必须大于 0")
    confidence_level = finite_number(
        interval["confidence_level"], f"{directory} confidence level"
    )
    lower_bound = finite_number(
        interval["lower_bound"], f"{directory} confidence lower bound"
    )
    upper_bound = finite_number(
        interval["upper_bound"], f"{directory} confidence upper bound"
    )
    metric_id = benchmark.get("full_id")
    if not isinstance(metric_id, str) or not metric_id:
        raise ValueError(f"{directory} 缺少 benchmark.full_id")

    return (
        {
            "id": metric_id,
            "canonical_estimate_source": estimate_source,
            "canonical_estimate_ns": rounded(canonical_estimate_ns),
            "confidence_interval_ns": {
                "confidence_level": confidence_level,
                "lower": rounded(lower_bound),
                "upper": rounded(upper_bound),
            },
            "sample_count": len(samples_ns),
            "normalized_batch_sample_p50_ns": rounded(
                percentile(samples_ns, 0.50)
            ),
            "normalized_batch_sample_p95_ns": rounded(
                percentile(samples_ns, 0.95)
            ),
        },
        samples_ns,
    )


def collect_run(
    criterion_home: Path,
    baseline: str,
    expected_ids: set[str],
    expected_sample_count: int,
) -> tuple[list[dict[str, Any]], dict[str, list[float]]]:
    """收集一个外层重复，并拒绝缺失或意外 workload。"""

    found: dict[str, tuple[dict[str, Any], list[float]]] = {}
    for benchmark_file in criterion_home.rglob("benchmark.json"):
        if benchmark_file.parent.name != baseline:
            continue
        metric, samples = parse_metric(benchmark_file.parent, expected_sample_count)
        metric_id = metric["id"]
        if metric_id in found:
            raise ValueError(f"重复 Criterion metric: {metric_id}")
        found[metric_id] = (metric, samples)

    actual_ids = set(found)
    if actual_ids != expected_ids:
        missing = sorted(expected_ids - actual_ids)
        unexpected = sorted(actual_ids - expected_ids)
        raise ValueError(
            f"Criterion 输出与固定 workload 不一致: missing={missing}, unexpected={unexpected}"
        )
    ordered = [found[metric_id][0] for metric_id in sorted(found)]
    samples_by_id = {metric_id: found[metric_id][1] for metric_id in sorted(found)}
    return ordered, samples_by_id


def benchmark_command(config: dict[str, Any], baseline: str) -> list[str]:
    """构造固定且可记录的 Criterion 命令。"""

    return [
        "cargo",
        "bench",
        "-p",
        config["package"],
        "--bench",
        config["bench"],
        "--locked",
        "--",
        "--warm-up-time",
        str(config["warm_up_seconds"]),
        "--measurement-time",
        str(config["measurement_seconds"]),
        "--sample-size",
        str(config["sample_size"]),
        "--noplot",
        "--save-baseline",
        baseline,
    ]


def console_safe_text(text: str, encoding: str) -> str:
    """把文本降级为目标终端可表示的字符集。"""

    return text.encode(encoding, errors="replace").decode(
        encoding, errors="replace"
    )


def write_console(text: str) -> None:
    """按当前终端编码安全镜像输出，UTF-8 原始日志不受影响。"""

    encoding = getattr(sys.stdout, "encoding", None) or "utf-8"
    safe_text = console_safe_text(text, encoding)
    sys.stdout.write(safe_text)
    sys.stdout.flush()


def run_and_tee(command: list[str], log_path: Path, env: dict[str, str]) -> None:
    """执行 benchmark，同时把完整输出保留为构建产物。"""

    with log_path.open("w", encoding="utf-8", newline="\n") as log:
        process = subprocess.Popen(
            command,
            cwd=ROOT,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
            bufsize=1,
        )
        assert process.stdout is not None
        for line in process.stdout:
            write_console(line)
            log.write(line)
        return_code = process.wait()
    if return_code != 0:
        raise RuntimeError(f"benchmark 命令失败 ({return_code}): {' '.join(command)}")


def command_output(command: list[str]) -> str:
    """采集不会包含 secret 的工具链元数据。"""

    return subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    ).stdout.strip()


def cpu_model() -> str:
    """读取不含主机身份的 CPU 型号。"""

    if os.environ.get("PROCESSOR_IDENTIFIER"):
        return os.environ["PROCESSOR_IDENTIFIER"]
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.is_file():
        for line in cpuinfo.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.lower().startswith("model name") and ":" in line:
                return line.split(":", 1)[1].strip()
    return platform.processor() or "unknown"


def locked_package_version(package: str) -> str:
    """从锁文件读取实际参与基准构建的 crate 版本。"""

    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    versions = sorted(
        {
            item["version"]
            for item in lock.get("package", [])
            if item.get("name") == package and isinstance(item.get("version"), str)
        }
    )
    if not versions:
        raise ValueError(f"Cargo.lock 中找不到 {package}")
    return ",".join(versions)


def environment_metadata(config: dict[str, Any]) -> dict[str, Any]:
    """生成跨运行比较所需的环境指纹。"""

    tracked_dirty = bool(
        command_output(["git", "status", "--porcelain", "--untracked-files=no"])
    )
    worktree_dirty = bool(command_output(["git", "status", "--porcelain"]))
    return {
        "git": {
            "commit": command_output(["git", "rev-parse", "HEAD"]),
            "tracked_dirty": tracked_dirty,
            "worktree_dirty": worktree_dirty,
        },
        "toolchain": {
            "rustc": command_output(["rustc", "-Vv"]),
            "cargo": command_output(["cargo", "-V"]),
            "criterion": locked_package_version("criterion"),
        },
        "host": {
            "os": platform.platform(),
            "architecture": platform.machine(),
            "cpu_model": cpu_model(),
            "logical_cpus": os.cpu_count(),
            "ci": os.environ.get("CI", "").lower() == "true",
            "runner_os": os.environ.get("RUNNER_OS"),
            "runner_arch": os.environ.get("RUNNER_ARCH"),
            "runner_environment": os.environ.get("RUNNER_ENVIRONMENT"),
            "runner_name": os.environ.get("RUNNER_NAME"),
            "runner_image_os": os.environ.get("ImageOS"),
            "runner_image_version": os.environ.get("ImageVersion"),
        },
        "build": {
            "cargo_features": config["cargo_features"],
            "rustflags": os.environ.get("RUSTFLAGS", ""),
        },
        "fingerprints": {
            "config_sha256": sha256(CONFIG_PATH),
            "benchmark_source_sha256": sha256(BENCH_SOURCE),
            "cargo_lock_sha256": sha256(ROOT / "Cargo.lock"),
        },
    }


def aggregate(
    config: dict[str, Any],
    runs: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    """按 metric 汇总三次外层重复。"""

    metric_config = {metric["id"]: metric for metric in config["metrics"]}
    summaries = []
    for metric_id in sorted(metric_config):
        run_metrics = [
            next(metric for metric in run["metrics"] if metric["id"] == metric_id)
            for run in runs
        ]
        canonical_estimates = [
            metric["canonical_estimate_ns"] for metric in run_metrics
        ]
        sample_p50s = [
            metric["normalized_batch_sample_p50_ns"] for metric in run_metrics
        ]
        sample_p95s = [
            metric["normalized_batch_sample_p95_ns"] for metric in run_metrics
        ]
        median_ns = statistics.median(canonical_estimates)
        mean_ns = statistics.fmean(canonical_estimates)
        summaries.append(
            {
                "id": metric_id,
                "class": metric_config[metric_id]["class"],
                "description": metric_config[metric_id]["description"],
                "unit": "ns/iteration",
                "run_canonical_estimates_ns": canonical_estimates,
                "median_run_canonical_ns": rounded(median_ns),
                "median_run_normalized_batch_sample_p50_ns": rounded(
                    statistics.median(sample_p50s)
                ),
                "median_run_normalized_batch_sample_p95_ns": rounded(
                    statistics.median(sample_p95s)
                ),
                "operations_per_second": rounded(1_000_000_000 / median_ns),
                "outer_spread_percent": rounded(
                    (max(canonical_estimates) - min(canonical_estimates))
                    / median_ns
                    * 100
                ),
                "outer_cv_percent": rounded(
                    statistics.pstdev(canonical_estimates) / mean_ns * 100
                ),
                "sample_count_per_run": [
                    metric["sample_count"] for metric in run_metrics
                ],
                "total_sample_count": sum(
                    metric["sample_count"] for metric in run_metrics
                ),
                "verdict": "observed",
            }
        )
    return summaries


def prepare_output(path: Path) -> Path:
    """创建新输出目录，拒绝覆盖历史原始数据。"""

    resolved = path if path.is_absolute() else ROOT / path
    if resolved.exists():
        if not resolved.is_dir():
            raise ValueError(f"输出路径不是目录: {resolved}")
        if any(resolved.iterdir()):
            raise ValueError(f"输出目录非空，拒绝覆盖历史数据: {resolved}")
    resolved.mkdir(parents=True, exist_ok=True)
    return resolved


def write_json(path: Path, payload: dict[str, Any]) -> None:
    """严格写 JSON，拒绝 NaN 与 Infinity。"""

    path.write_text(
        json.dumps(
            payload,
            allow_nan=False,
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def write_failure_manifest(
    output: Path,
    config: dict[str, Any] | None,
    started_at: datetime,
    total_started: float,
    runs: list[dict[str, Any]],
    current_run: dict[str, Any] | None,
    environment: dict[str, Any] | None,
    error: Exception,
) -> Path:
    """在采集失败时保留已完成证据与失败上下文。"""

    manifest = {
        "schema_version": 1,
        "suite": config.get("suite") if config else "yang-base/runtime_baseline",
        "mode": "shadow",
        "status": "failed",
        "blocking": False,
        "started_at_utc": started_at.isoformat(),
        "failed_at_utc": datetime.now(timezone.utc).isoformat(),
        "duration_seconds": rounded(time.perf_counter() - total_started),
        "workload_contract": config,
        "environment": environment,
        "completed_runs": runs,
        "current_run": current_run,
        "error": {
            "type": type(error).__name__,
            "message": str(error),
        },
    }
    failure_path = output / "failure.json"
    write_json(failure_path, manifest)
    return failure_path


def collect(config: dict[str, Any], output: Path) -> Path:
    """执行固定次数采集并写出 summary.json。"""

    output = prepare_output(output)
    expected_ids = {metric["id"] for metric in config["metrics"]}
    runs: list[dict[str, Any]] = []
    started_at = datetime.now(timezone.utc)
    total_started = time.perf_counter()
    environment: dict[str, Any] | None = None
    current_run: dict[str, Any] | None = None

    try:
        environment = environment_metadata(config)
        for index in range(1, config["repetitions"] + 1):
            run_name = f"run-{index:02d}"
            baseline = f"shadow-{index:02d}"
            run_directory = output / "runs" / run_name
            criterion_home = run_directory / "criterion"
            criterion_home.mkdir(parents=True)
            command = benchmark_command(config, baseline)
            env = os.environ.copy()
            env["CRITERION_HOME"] = str(criterion_home)
            env["CARGO_TERM_COLOR"] = "never"
            run_started = time.perf_counter()
            current_run = {
                "index": index,
                "name": run_name,
                "baseline": baseline,
                "command": command,
                "criterion_home": str(criterion_home.relative_to(output)),
                "console_log": str(
                    (run_directory / "stdout-stderr.log").relative_to(output)
                ),
            }
            run_and_tee(command, run_directory / "stdout-stderr.log", env)
            metrics, _samples_by_id = collect_run(
                criterion_home,
                baseline,
                expected_ids,
                config["sample_size"],
            )
            current_run["duration_seconds"] = rounded(
                time.perf_counter() - run_started
            )
            current_run["metrics"] = metrics
            runs.append(current_run)
            current_run = None

        summary = {
            "schema_version": 1,
            "suite": config["suite"],
            "mode": "shadow",
            "status": "completed",
            "blocking": False,
            "started_at_utc": started_at.isoformat(),
            "duration_seconds": rounded(time.perf_counter() - total_started),
            "workload_contract": config,
            "statistics": {
                "canonical_per_run": (
                    "criterion slope.point_estimate; fallback mean.point_estimate"
                ),
                "outer_canonical_aggregation": "median across run canonical estimates",
                "normalized_batch_sample": "sample.times_ns / sample.iters",
                "percentile_method": "nearest-rank",
                "outer_percentile_aggregation": (
                    "median across corresponding per-run sample percentiles"
                ),
            },
            "policy": {
                "blocking": False,
                "regression_threshold_percent": None,
                "conclusion": "not_evaluated",
                "note": "B-01 只采集数据；校准 runner 噪声前不得据此通过或阻断变更。",
            },
            "environment": environment,
            "runs": runs,
            "summary": aggregate(config, runs),
        }
        summary_path = output / "summary.json"
        write_json(summary_path, summary)
        print(f"performance shadow summary: {summary_path}")
        return summary_path
    except Exception as error:
        failure_path = write_failure_manifest(
            output,
            config,
            started_at,
            total_started,
            runs,
            current_run,
            environment,
            error,
        )
        print(f"performance shadow failure manifest: {failure_path}", file=sys.stderr)
        raise


def write_fixture(
    directory: Path,
    baseline: str,
    metric_id: str,
    sample_count: int = 50,
    slope: bool = True,
) -> None:
    """写入最小 Criterion fixture，供解析器对抗性自测。"""

    metric_dir = directory / metric_id.replace("/", "_") / baseline
    metric_dir.mkdir(parents=True)
    (metric_dir / "benchmark.json").write_text(
        json.dumps({"full_id": metric_id}), encoding="utf-8"
    )
    estimate = {
        "point_estimate": 11.0,
        "confidence_interval": {
            "confidence_level": 0.95,
            "lower_bound": 10.0,
            "upper_bound": 12.0,
        },
    }
    (metric_dir / "estimates.json").write_text(
        json.dumps({"slope": estimate if slope else None, "mean": estimate}),
        encoding="utf-8",
    )
    sample_values = [float(index) for index in range(1, sample_count + 1)]
    (metric_dir / "sample.json").write_text(
        json.dumps({"iters": [1.0] * sample_count, "times": sample_values}),
        encoding="utf-8",
    )
    (metric_dir / "tukey.json").write_text("{}", encoding="utf-8")


def self_test() -> None:
    """证明配置漂移、缺失/意外 metric 与覆盖历史数据都会被拒绝。"""

    config = load_config()
    expected_ids = {metric["id"] for metric in config["metrics"]}
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        criterion_home = root / "criterion"
        for metric_index, metric_id in enumerate(sorted(expected_ids)):
            write_fixture(
                criterion_home,
                "shadow-01",
                metric_id,
                slope=metric_index != 0,
            )
        metrics, samples = collect_run(
            criterion_home,
            "shadow-01",
            expected_ids,
            config["sample_size"],
        )
        assert len(metrics) == len(expected_ids)
        assert all(
            metric["normalized_batch_sample_p50_ns"] == 25.0
            for metric in metrics
        )
        assert all(
            metric["normalized_batch_sample_p95_ns"] == 48.0
            for metric in metrics
        )
        assert any(
            metric["canonical_estimate_source"] == "mean" for metric in metrics
        )
        assert all(len(values) == config["sample_size"] for values in samples.values())

        synthetic_runs = []
        for index, (canonical, sample_p50, sample_p95) in enumerate(
            [(10.0, 20.0, 40.0), (11.0, 25.0, 45.0), (13.0, 30.0, 50.0)],
            start=1,
        ):
            run_metrics = json.loads(json.dumps(metrics))
            for metric in run_metrics:
                metric["canonical_estimate_ns"] = canonical
                metric["normalized_batch_sample_p50_ns"] = sample_p50
                metric["normalized_batch_sample_p95_ns"] = sample_p95
            synthetic_runs.append({"index": index, "metrics": run_metrics})
        summary = aggregate(config, synthetic_runs)[0]
        assert summary["median_run_canonical_ns"] == 11.0
        assert summary["median_run_normalized_batch_sample_p50_ns"] == 25.0
        assert summary["median_run_normalized_batch_sample_p95_ns"] == 45.0
        assert summary["outer_spread_percent"] == 27.272727
        assert summary["operations_per_second"] == 90_909_090.909091
        assert summary["sample_count_per_run"] == [50, 50, 50]
        assert summary["total_sample_count"] == 150

        missing_ids = set(expected_ids)
        missing_ids.add("missing/metric")
        try:
            collect_run(
                criterion_home,
                "shadow-01",
                missing_ids,
                config["sample_size"],
            )
        except ValueError:
            pass
        else:
            raise AssertionError("采集器未拒绝缺失 metric")

        write_fixture(criterion_home, "shadow-01", "unexpected/metric")
        try:
            collect_run(
                criterion_home,
                "shadow-01",
                expected_ids,
                config["sample_size"],
            )
        except ValueError:
            pass
        else:
            raise AssertionError("采集器未拒绝意外 metric")

        invalid_count_home = root / "invalid-count"
        write_fixture(invalid_count_home, "shadow-01", "only/metric", sample_count=49)
        try:
            collect_run(invalid_count_home, "shadow-01", {"only/metric"}, 50)
        except ValueError:
            pass
        else:
            raise AssertionError("采集器未拒绝错误样本数")

        try:
            finite_number(math.nan, "fixture")
        except ValueError:
            pass
        else:
            raise AssertionError("采集器未拒绝非有限数值")

        try:
            write_json(root / "invalid.json", {"value": math.inf})
        except ValueError:
            pass
        else:
            raise AssertionError("JSON 写入器未拒绝 Infinity")

        config_text = CONFIG_PATH.read_text(encoding="utf-8")
        for invalid_number in ("nan", "inf"):
            invalid_config = root / f"invalid-{invalid_number}.toml"
            invalid_config.write_text(
                config_text.replace(
                    "warm_up_seconds = 1.0",
                    f"warm_up_seconds = {invalid_number}",
                ),
                encoding="utf-8",
            )
            try:
                load_config(invalid_config)
            except ValueError:
                pass
            else:
                raise AssertionError(
                    f"配置校验未拒绝 warm_up_seconds = {invalid_number}"
                )

        output = root / "existing-output"
        output.mkdir()
        (output / "history.json").write_text("{}", encoding="utf-8")
        try:
            prepare_output(output)
        except ValueError:
            pass
        else:
            raise AssertionError("采集器未拒绝覆盖历史数据")

    assert benchmark_command(config, "shadow-01")[-2:] == [
        "--save-baseline",
        "shadow-01",
    ]
    assert config["repetitions"] == 3
    assert config["sample_size"] == 50
    assert console_safe_text("1 µs", "ascii") == "1 ?s"
    print("performance shadow self-test passed")


def default_output() -> Path:
    """为本地采集生成不会覆盖历史结果的默认目录。"""

    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return ROOT / "target" / "performance-shadow" / timestamp


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0

    output = args.output or default_output()
    try:
        config = load_config()
    except (OSError, ValueError) as error:
        started_at = datetime.now(timezone.utc)
        total_started = time.perf_counter()
        try:
            resolved_output = prepare_output(output)
            write_failure_manifest(
                resolved_output,
                None,
                started_at,
                total_started,
                [],
                None,
                None,
                error,
            )
        except (OSError, ValueError):
            pass
        raise
    collect(config, output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"performance shadow failed: {error}", file=sys.stderr)
        raise SystemExit(1) from None
