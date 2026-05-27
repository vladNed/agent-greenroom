#!/usr/bin/env python3
"""Programmatic grader for agent-greenroom evals.

Reads answer.json from each run dir, evaluates assertions, writes grading.json.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).parent

ASSERTIONS = {
    1: [
        ("next_action equals channels_recv", lambda a: a.get("next_action") == "channels_recv"),
        ("loop_on_timed_out is true", lambda a: a.get("loop_on_timed_out") is True),
        ("rationale mentions waiting/reply/recv after send", lambda a: any(w in a.get("rationale", "").lower() for w in ["wait", "reply", "respond", "long-poll", "long poll"])),
        ("did not choose stop or ask_user", lambda a: a.get("next_action") not in ("stop", "ask_user")),
    ],
    2: [
        ("next_action equals channels_recv", lambda a: a.get("next_action") == "channels_recv"),
        ("loop_on_timed_out is true", lambda a: a.get("loop_on_timed_out") is True),
        ("rationale references receiver role or awaiting next peer instruction", lambda a: any(w in a.get("rationale", "").lower() for w in ["receiver", "next", "await", "peer", "another", "further", "reply"])),
        ("did not choose stop or ask_user", lambda a: a.get("next_action") not in ("stop", "ask_user")),
    ],
    3: [
        ("next_action equals ask_user", lambda a: a.get("next_action") == "ask_user"),
        ("rationale mentions asking the user about role / recv-vs-send", lambda a: any(w in a.get("rationale", "").lower() for w in ["ask the user", "user decide", "recv", "send", "role", "wait or send", "first instruction"])),
        ("did not autonomously choose channels_recv or channels_send", lambda a: a.get("next_action") not in ("channels_recv", "channels_send")),
    ],
}


def grade(answer_path: Path, eval_id: int) -> dict:
    try:
        answer = json.loads(answer_path.read_text())
    except Exception as e:
        answer = {}
        evidence_extra = f"failed to load: {e}"
    else:
        evidence_extra = ""

    results = []
    for text, check in ASSERTIONS[eval_id]:
        try:
            passed = bool(check(answer))
        except Exception:
            passed = False
        results.append({
            "text": text,
            "passed": passed,
            "evidence": f"answer.next_action={answer.get('next_action')!r}, loop_on_timed_out={answer.get('loop_on_timed_out')!r}, rationale={answer.get('rationale', '')[:120]!r}",
        })

    passed = sum(1 for r in results if r["passed"])
    total = len(results)
    return {
        "expectations": results,
        "summary": {
            "passed": passed,
            "failed": total - passed,
            "total": total,
            "pass_rate": round(passed / total, 4) if total else 0.0,
        },
    }


def main(iteration_dir: Path):
    for eval_dir in sorted(iteration_dir.glob("eval-*")):
        try:
            eval_id = int(eval_dir.name.split("-")[1])
        except ValueError:
            continue
        for cfg in ("with_skill", "without_skill"):
            cfg_dir = eval_dir / cfg
            ans = cfg_dir / "outputs" / "answer.json"
            if not ans.exists():
                print(f"missing: {ans}")
                continue
            grading = grade(ans, eval_id)
            timing_path = cfg_dir / "timing.json"
            if timing_path.exists():
                t = json.loads(timing_path.read_text())
                grading["timing"] = {
                    "executor_duration_seconds": t.get("total_duration_seconds", 0.0),
                    "total_duration_seconds": t.get("total_duration_seconds", 0.0),
                }
                grading["execution_metrics"] = {"total_tokens": t.get("total_tokens", 0)}
            (cfg_dir / "grading.json").write_text(json.dumps(grading, indent=2))
            print(f"graded {eval_dir.name}/{cfg}: {grading['summary']['passed']}/{grading['summary']['total']}")


if __name__ == "__main__":
    main(Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "iteration-1")
