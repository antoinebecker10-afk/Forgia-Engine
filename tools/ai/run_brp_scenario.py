#!/usr/bin/env python3
"""Lance et vérifie un scénario agentique sur le vrai runtime Forgia via BRP."""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SCENARIO = ROOT / "tools/ai/scenarios/roguelite_first_contact.json"
BRP_URL = "http://127.0.0.1:15702/"


def brp(method: str, params: dict | None = None, request_id: int = 1) -> dict:
    payload = {"jsonrpc": "2.0", "id": request_id, "method": method}
    if params is not None:
        payload["params"] = params
    request = urllib.request.Request(
        BRP_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=2) as response:
        body = json.load(response)
    if "error" in body:
        raise RuntimeError(f"BRP {method}: {body['error']}")
    return body["result"]


def wait_snapshot(predicate, timeout_s: float, label: str) -> dict:
    deadline = time.monotonic() + timeout_s
    last_error = "aucune réponse"
    while time.monotonic() < deadline:
        try:
            snapshot = brp("forgia.scenario.snapshot")
            if predicate(snapshot):
                return snapshot
            last_error = json.dumps(snapshot, ensure_ascii=False)
        except (OSError, RuntimeError, urllib.error.URLError) as error:
            last_error = str(error)
        time.sleep(0.25)
    raise TimeoutError(f"timeout {label}: {last_error}")


def distance(a: list[float], b: list[float]) -> float:
    return math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)))


def read_sensor(path: Path) -> dict:
    try:
        return {"path": path.name, "value": json.loads(path.read_text(encoding="utf-8"))}
    except FileNotFoundError:
        return {"path": path.name, "error": "missing"}
    except (OSError, json.JSONDecodeError) as error:
        return {"path": path.name, "error": str(error)}


def target_by_id(snapshot: dict, entity: int) -> dict | None:
    return next((target for target in snapshot.get("targets", []) if target["entity"] == entity), None)


def run_first_contact(scenario: dict, report: dict, before: dict) -> list[tuple[str, bool, object]]:
    for index, action in enumerate(scenario["actions"], start=1):
        accepted = brp("forgia.scenario.act", action, index + 10)
        report["actions"].append({"request": action, "response": accepted})
        after_action = wait_snapshot(
            lambda s: s.get("driver", {}).get("idle") is True,
            scenario["action_timeout_s"],
            f"action {action['action']}",
        )
        report["snapshots"].append(
            {"label": f"after_{action['action']}", "value": after_action}
        )

    after = report["snapshots"][-1]["value"]
    expected = scenario["assertions"]
    moved = distance(before["player"]["position"], after["player"]["position"])
    new_shots = after["total_shots"] - before["total_shots"]
    return [
        ("game_mode", after["game_mode"] == expected["game_mode"], after["game_mode"]),
        ("run_state", str(after.get("run_state", "")).startswith(expected["run_state_prefix"]), after.get("run_state")),
        ("player_moved", moved >= expected["minimum_distance_m"], moved),
        ("shot_observed", new_shots >= expected["minimum_new_shots"], new_shots),
    ]


def run_combat_first_kill(scenario: dict, report: dict, before: dict) -> list[tuple[str, bool, object]]:
    candidates = [target for target in before.get("targets", []) if target["health"]["current"] > 0]
    if not candidates:
        raise RuntimeError("aucune cible vivante observable")
    target = min(candidates, key=lambda item: (item["health"]["current"], item.get("distance_m") or 9999))
    entity = target["entity"]
    initial_health = target["health"]["current"]
    minimum_health = initial_health
    last = before

    for index in range(scenario["maximum_fire_actions"]):
        aim_request = {"entity": entity}
        aimed = brp("forgia.scenario.aim_at", aim_request, 100 + index * 2)
        report["actions"].append({"request": {"action": "aim_at", **aim_request}, "response": aimed})
        time.sleep(scenario.get("aim_settle_s", 0.1))
        fire_request = {"action": "fire", "frames": scenario["fire_frames"]}
        fired = brp("forgia.scenario.act", fire_request, 101 + index * 2)
        report["actions"].append({"request": fire_request, "response": fired})
        last = wait_snapshot(
            lambda s: s.get("driver", {}).get("idle") is True,
            scenario["action_timeout_s"],
            f"tir {index + 1}",
        )
        current = target_by_id(last, entity)
        if current is not None:
            minimum_health = min(minimum_health, current["health"]["current"])
        else:
            # Une cible one-shot disparaît avant le snapshot suivant : son dernier
            # état observable est nécessairement HP=0, confirmé par le hit létal.
            minimum_health = 0
        report["snapshots"].append({"label": f"after_fire_{index + 1}", "value": last})
        if last.get("kills", 0) > before.get("kills", 0):
            break
        time.sleep(scenario.get("between_shots_s", 0.2))

    after = wait_snapshot(
        lambda s: s.get("enemies", 0) < before.get("enemies", 0)
        and s.get("kills", 0) > before.get("kills", 0),
        scenario["death_timeout_s"],
        "mort et killfeed",
    )
    report["snapshots"].append({"label": "after_death", "value": after})
    loot_before = before["loot"]
    loot_after = after["loot"]
    loot_evidence = (
        loot_after["pickups"] > loot_before["pickups"]
        or loot_after["souls_total_collected"] > loot_before["souls_total_collected"]
    )
    remaining_ids = {item["entity"] for item in after.get("targets", [])}
    removed_targets = [
        {"entity": item["entity"], "name": item.get("name")}
        for item in before.get("targets", [])
        if item["entity"] not in remaining_ids
    ]
    damage_evidence = after["hits_with_damage"] > before["hits_with_damage"]
    checks = [
        ("target_acquired", True, {"entity": entity, "name": target.get("name")}),
        ("damage_observed", damage_evidence, {"selected_target_before": initial_health, "selected_target_minimum": minimum_health}),
        ("hit_observed", after["hits_with_damage"] > before["hits_with_damage"], after["hits_with_damage"] - before["hits_with_damage"]),
        ("kill_observed", after["kills"] > before["kills"], after["kills"] - before["kills"]),
        ("killed_entity_removed", len(removed_targets) >= 1, removed_targets),
        ("loot_observed", loot_evidence, {"before": loot_before, "after": loot_after}),
        ("wave_progressed", after["wave"]["bots_alive"] < before["wave"]["bots_alive"], {"before": before["wave"], "after": after["wave"]}),
    ]
    return checks


def expedition_body(snapshot: dict) -> dict | None:
    return next(
        (item for item in snapshot.get("avatar_animation", []) if "stylized_male" in item.get("model", "")),
        None,
    )


def run_expedition_animation_audit(scenario: dict, report: dict, before: dict) -> list[tuple[str, bool, object]]:
    checks: list[tuple[str, bool, object]] = []
    idle = expedition_body(before)
    checks.append(("idle_clip", idle is not None and idle.get("playing_clip") == "Idle", idle))

    for index, step in enumerate(scenario["animation_steps"], start=1):
        request = {"action": step["action"], "frames": step["frames"]}
        accepted = brp("forgia.scenario.act", request, 300 + index)
        started_at = time.monotonic()
        report["actions"].append({"request": request, "response": accepted})
        wait_snapshot(
            lambda s: s.get("driver", {}).get("active") is not None,
            scenario["action_timeout_s"],
            f"début {step['action']}",
        )
        earliest_sample = started_at + step.get("sample_after_s", scenario["sample_after_s"])
        transition_deadline = started_at + scenario["transition_timeout_ms"] / 1000.0
        active = None
        while time.monotonic() < transition_deadline:
            candidate = brp("forgia.scenario.snapshot", request_id=500 + index)
            body = expedition_body(candidate)
            if (
                time.monotonic() >= earliest_sample
                and body is not None
                and body.get("requested_state") == step["expected_state"]
            ):
                active = candidate
                break
            time.sleep(0.015)
        if active is None:
            active = brp("forgia.scenario.snapshot", request_id=600 + index)
        transition_ms = round((time.monotonic() - started_at) * 1000, 1)
        report["snapshots"].append({"label": f"during_{step['action']}", "value": active})
        body = expedition_body(active)
        state_ok = body is not None and body.get("requested_state") == step["expected_state"]
        clip_ok = body is not None and body.get("playing_clip") == step["expected_clip"]
        checks.append((f"{step['action']}_state", state_ok, body))
        checks.append((f"{step['action']}_clip", clip_ok, body))
        checks.append((
            f"{step['action']}_responsive",
            state_ok and transition_ms <= scenario["maximum_transition_ms"],
            {"transition_ms": transition_ms, "maximum_ms": scenario["maximum_transition_ms"]},
        ))
        stopped = brp("forgia.scenario.stop", request_id=700 + index)
        report["actions"].append({"request": {"action": "stop"}, "response": stopped})
        wait_snapshot(
            lambda s: s.get("driver", {}).get("idle") is True,
            scenario["action_timeout_s"],
            f"fin {step['action']}",
        )
        if "jump" in step["action"]:
            wait_snapshot(
                lambda s: (s.get("locomotion") or {}).get("grounded") is True,
                scenario["action_timeout_s"],
                f"atterrissage après {step['action']}",
            )
        time.sleep(scenario["settle_between_actions_s"])

    final = brp("forgia.scenario.snapshot", request_id=900)
    report["snapshots"].append({"label": "final_idle", "value": final})
    final_body = expedition_body(final)
    checks.append(("returns_to_idle", final_body is not None and final_body.get("playing_clip") == "Idle", final_body))
    checks.append(("animation_never_restarted", final_body is not None and final_body.get("restarts") == 0, final_body))
    checks.append(("player_moved", distance(before["player"]["position"], final["player"]["position"]) > 1.0, {"before": before["player"], "after": final["player"]}))
    return checks


def run_expedition_first_camp_walk(scenario: dict, report: dict, before: dict) -> list[tuple[str, bool, object]]:
    camp = scenario.get("camp", "camp_1")
    accepted = brp("forgia.scenario.follow_camp", {"camp": camp}, request_id=1000)
    report["actions"].append({"request": {"action": "follow_camp", "camp": camp}, "response": accepted})
    deadline = time.monotonic() + scenario["path_timeout_s"]
    samples: list[dict] = []
    last = before
    while time.monotonic() < deadline:
        last = brp("forgia.scenario.snapshot", request_id=1001 + len(samples))
        samples.append(last)
        follower = last.get("path_follower") or {}
        if follower.get("completed") or follower.get("failure"):
            break
        time.sleep(scenario.get("sample_interval_s", 0.1))
    report["path_samples"] = samples
    report["snapshots"].append({"label": "at_first_camp", "value": last})
    follower = last.get("path_follower") or {}
    bodies = [expedition_body(sample) for sample in samples]
    requested = [body.get("requested_state") for body in bodies if body is not None]
    landing_count = sum(state == "Atterrissage" for state in requested)
    unexpected = sorted({state for state in requested if state not in {"Marche", "Idle"}})
    speeds = [sample.get("player_horizontal_speed_mps", 0.0) for sample in samples[:-1]]
    moving_samples = sum(speed >= scenario["minimum_walk_speed_mps"] for speed in speeds)
    return [
        ("path_completed", follower.get("completed") is True, follower),
        ("path_has_no_failure", follower.get("failure") is None, follower.get("failure")),
        ("camp_radius_reached", follower.get("distance_to_camp_m", math.inf) <= scenario["maximum_camp_distance_m"], follower.get("distance_to_camp_m")),
        ("no_false_landing", landing_count == 0, landing_count),
        ("only_walk_or_idle", not unexpected, unexpected),
        ("continuous_walk_observed", moving_samples >= scenario["minimum_moving_samples"], moving_samples),
        # Arriver au camp ne prouve rien sur la ROUTE : le KCC glisse autour de
        # ce qui bloque. Un obstacle poussé sans gagner un centimètre est relevé
        # avec sa position — c'est là qu'il faudra aller voir sous Blender.
        ("path_never_blocked", not follower.get("blocked_at"), follower.get("blocked_at")),
    ]


def run() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("scenario", nargs="?", type=Path, default=DEFAULT_SCENARIO)
    parser.add_argument("--attach", action="store_true", help="utilise un jeu déjà lancé")
    args = parser.parse_args()
    scenario_path = args.scenario.resolve()
    scenario = json.loads(scenario_path.read_text(encoding="utf-8"))

    output_dir = ROOT / "target/forgia_agent"
    output_dir.mkdir(parents=True, exist_ok=True)
    stamp = time.strftime("%Y%m%d-%H%M%S")
    report_path = output_dir / f"{scenario['name']}-{stamp}.json"
    log_path = output_dir / f"{scenario['name']}-{stamp}.log"
    process = None
    log_file = None
    report = {
        "schema_version": 1,
        "scenario": scenario,
        "scenario_path": str(scenario_path.relative_to(ROOT)),
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "outcome": "inconclusive",
        "snapshots": [],
        "actions": [],
        "assertions": [],
        "sensors": [],
        "process_log": str(log_path.relative_to(ROOT)),
    }

    try:
        if not args.attach:
            env = os.environ.copy()
            env["FORGIA_BOOT_MODE"] = scenario["boot_mode"]
            command = ["cargo", "run", "-p", "forgia", "--features", "dev-brp", "--"]
            report["launch_command"] = command
            log_file = log_path.open("w", encoding="utf-8")
            process = subprocess.Popen(
                command,
                cwd=ROOT,
                env=env,
                stdout=log_file,
                stderr=subprocess.STDOUT,
            )

        expected = scenario["assertions"]
        workflow = scenario.get("workflow", "first_contact")

        def runtime_ready(snapshot: dict) -> bool:
            base_ready = (
                snapshot.get("game_mode") == expected["game_mode"]
                and str(snapshot.get("run_state", "")).startswith(expected["run_state_prefix"])
                and snapshot.get("player") is not None
            )
            if workflow not in {"expedition_animation_audit", "expedition_first_camp_walk"}:
                return base_ready
            body = expedition_body(snapshot)
            # Un KCC immobile peut publier `grounded=false` jusqu'à sa prochaine
            # translation sans que l'avatar flotte. La readiness certifie le
            # chargement et le binding ; les gestes testent ensuite le contact.
            return (
                base_ready
                and body is not None
                and body.get("bound") is True
                and body.get("available_clips", 0) > 0
                and body.get("playing_clip") == "Idle"
            )

        before = wait_snapshot(
            runtime_ready,
            scenario["ready_timeout_s"],
            "run jouable",
        )
        report["snapshots"].append({"label": "before", "value": before})

        if workflow == "first_contact":
            checks = run_first_contact(scenario, report, before)
        elif workflow == "combat_first_kill":
            checks = run_combat_first_kill(scenario, report, before)
        elif workflow == "expedition_animation_audit":
            checks = run_expedition_animation_audit(scenario, report, before)
        elif workflow == "expedition_first_camp_walk":
            checks = run_expedition_first_camp_walk(scenario, report, before)
        else:
            raise ValueError(f"workflow inconnu: {workflow}")
        report["assertions"] = [
            {"name": name, "passed": passed, "observed": observed}
            for name, passed, observed in checks
        ]
        report["sensors"] = [read_sensor(ROOT / name) for name in scenario["evidence_sensors"]]
        report["outcome"] = "pass" if all(passed for _, passed, _ in checks) else "fail"
    except Exception as error:  # Le rapport est la preuve, y compris sur panne de harnais.
        report["error"] = f"{type(error).__name__}: {error}"
        report["sensors"] = [read_sensor(ROOT / name) for name in scenario["evidence_sensors"]]
    finally:
        report["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        # Sur `--attach`, le jeu SURVIT au scénario : un run interrompu en pleine
        # action laisserait sinon le joueur courir tout seul, et le défaut suivant
        # serait diagnostiqué sur une partie pilotée par un fantôme.
        if process is None:
            try:
                brp("forgia.scenario.release_all")
            except (OSError, RuntimeError, urllib.error.URLError):
                pass
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
        if log_file is not None:
            log_file.close()

    print(report_path)
    return 0 if report["outcome"] == "pass" else 1


if __name__ == "__main__":
    sys.exit(run())
