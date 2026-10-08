from common import *
from concurrent.futures import ThreadPoolExecutor
import statistics


def run(i):
    cmd = command()
    start = time.perf_counter()
    result = create(cmd, "qa_actor_a" if i % 2 == 0 else "qa_actor_b")
    assert result[0] == "created"
    return (time.perf_counter() - start) * 1000


start = time.perf_counter()
with ThreadPoolExecutor(max_workers=2) as pool:
    samples = list(pool.map(run, range(40)))
data = dict(
    samples=40,
    workers=2,
    actors=2,
    total_ms=(time.perf_counter() - start) * 1000,
    median_ms=statistics.median(samples),
    p95_ms=sorted(samples)[37],
    max_ms=max(samples),
    conditions="Warm loopback, two independent connections at a time; includes fixture subprocess and validation; no product threshold",
)
(ROOT / "concurrent-measurements.json").write_text(json.dumps(data, indent=2))
rec("C04.concurrent-observation", detail=data, classification="descriptive experiment")
(ROOT / "concurrent-results.json").write_text(json.dumps(RESULTS, indent=2))
