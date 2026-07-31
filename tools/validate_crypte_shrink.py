#!/usr/bin/env python3
"""Validate shrunk crypte route contract (subset of Rust checks)."""
from __future__ import annotations

import math
import tomllib
from pathlib import Path

g = tomllib.loads(Path("assets/genomes/arena_test_crypte_vertical.toml").read_text(encoding="utf-8"))
rooms = {r["id"]: r for r in g["rooms"]}
errors: list[str] = []


def contains(room, x, z, eps=0.05) -> bool:
    c, s = room["center"], room["size"]
    return abs(x - c[0]) <= s[0] * 0.5 + eps and abs(z - c[2]) <= s[2] * 0.5 + eps


def slope(a, b) -> float:
    run = math.hypot(b[0] - a[0], b[2] - a[2])
    return abs(math.degrees(math.atan2(b[1] - a[1], run))) if run > 1e-6 else 90.0


def local_xz(p, blk):
    dx, dz = p[0] - blk["pos"][0], p[2] - blk["pos"][2]
    yaw = math.radians(blk.get("yaw_deg", 0.0))
    c, s = math.cos(yaw), math.sin(yaw)
    return (c * dx - s * dz, s * dx + c * dz)


def seg_hits_aabb(p, q, hx, hz) -> bool:
    for i in range(21):
        t = i / 20
        x = p[0] + (q[0] - p[0]) * t
        z = p[1] + (q[1] - p[1]) * t
        if abs(x) <= hx and abs(z) <= hz:
            return True
    return False


half_x = g["arena"]["size_x"] * 0.5
half_z = g["arena"]["size_z"] * 0.5

for route in g["routes"]:
    path = route["path"]
    p0, p1 = path[0], path[-1]
    if not contains(rooms[route["from"]], p0[0], p0[2]) or not contains(
        rooms[route["to"]], p1[0], p1[2]
    ):
        errors.append(f"endpoints {route['id']}")
    for a, b in zip(path, path[1:]):
        if abs(a[0]) > half_x or abs(a[2]) > half_z or abs(b[0]) > half_x or abs(b[2]) > half_z:
            errors.append(f"oob {route['id']}")
        if abs(a[1] - b[1]) > 0.25:
            sec = route.get("ramp_section", "")
            ramps = [r for r in g.get("ramps", []) if r.get("section") == sec]
            ok = False
            for r in ramps:
                fr, to = r["from"], r["to"]
                if (math.dist(a, fr) < 0.35 and math.dist(b, to) < 0.35) or (
                    math.dist(a, to) < 0.35 and math.dist(b, fr) < 0.35
                ):
                    ok = True
                    if slope(fr, to) > 25.1:
                        errors.append(f"slope {route['id']} {slope(fr, to):.1f}")
            if not ok:
                errors.append(f"no ramp {route['id']} {a}->{b}")
        for blk in g["blocks"]:
            if blk.get("role") not in ("wall", "cover", "perch"):
                continue
            by, bh = blk["pos"][1], blk["size"][1]
            if max(a[1], b[1]) < by - 0.05 or min(a[1], b[1]) > by + bh + 0.05:
                continue
            rad = 0.3
            hx = blk["size"][0] * 0.5 + rad
            hz = blk["size"][2] * 0.5 + rad
            if seg_hits_aabb(local_xz(a, blk), local_xz(b, blk), hx, hz):
                errors.append(f"{route['id']} hits {blk.get('section')}@{blk['pos']}")
                break

for room in g["rooms"]:
    d = math.hypot(room["size"][0], room["size"][2])
    if d > 34.5:
        errors.append(f"diag {room['id']} {d:.1f}")

print(
    f"rooms={len(g['rooms'])} routes={len(g['routes'])} blocks={len(g['blocks'])} "
    f"world={g['arena']['size_x']}x{g['arena']['size_z']}"
)
print(f"ERRORS {len(errors)}")
for e in errors:
    print(" -", e)
raise SystemExit(1 if errors else 0)
