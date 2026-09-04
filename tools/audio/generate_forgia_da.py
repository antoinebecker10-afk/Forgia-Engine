#!/usr/bin/env python3
"""Génère le pack audio original « forge fantastique cartoon » de Forgia.

Zéro sample externe : oscillateurs, bruit déterministe et enveloppes uniquement.
Les WAV masters restent dans target/audio-masters ; les OGG livrables vont dans
assets/audio/forgia_original. Requiert seulement Python 3 et ffmpeg.
"""

from __future__ import annotations

import argparse
import math
import random
import shutil
import struct
import subprocess
import wave
from array import array
from pathlib import Path

SR = 48_000
TAU = math.tau
SEED = 0xF0A61A


def envelope(t: float, duration: float, attack: float, decay: float = 1.0) -> float:
    if t < 0.0 or t >= duration:
        return 0.0
    a = min(1.0, t / max(attack, 1e-5))
    return a * ((1.0 - t / duration) ** decay)


def softclip(x: float) -> float:
    return math.tanh(x * 1.35) / math.tanh(1.35)


def normalize(samples: array, peak: float = 0.92) -> array:
    high = max((abs(x) for x in samples), default=1.0)
    gain = peak / max(high, 1e-7)
    # Pas de saturation globale : elle ajoutait des harmoniques à chaque son et
    # rendait le pack agressif après quelques minutes.
    return array("f", (max(-1.0, min(1.0, x * gain)) for x in samples))


def mono(duration: float) -> array:
    return array("f", [0.0]) * int(duration * SR)


def stereo(duration: float) -> tuple[array, array]:
    return mono(duration), mono(duration)


def add_tone(buf: array, start: float, duration: float, freq: float, amp: float,
             attack: float = 0.002, decay: float = 2.0, sweep: float = 0.0,
             phase: float = 0.0) -> None:
    begin = int(start * SR)
    count = min(int(duration * SR), len(buf) - begin)
    p = phase
    for i in range(max(0, count)):
        t = i / SR
        f = max(18.0, freq + sweep * (t / max(duration, 1e-6)))
        p += TAU * f / SR
        buf[begin + i] += math.sin(p) * amp * envelope(t, duration, attack, decay)


def add_noise(buf: array, rng: random.Random, start: float, duration: float, amp: float,
              attack: float = 0.001, decay: float = 2.0, lowpass: float = 0.2,
              highpass: float = 0.0) -> None:
    begin = int(start * SR)
    count = min(int(duration * SR), len(buf) - begin)
    lp = 0.0
    prev_lp = 0.0
    for i in range(max(0, count)):
        raw = rng.uniform(-1.0, 1.0)
        lp += lowpass * (raw - lp)
        value = lp
        if highpass > 0.0:
            value = lp - prev_lp * highpass
            prev_lp = lp
        t = i / SR
        buf[begin + i] += value * amp * envelope(t, duration, attack, decay)


def add_metal(buf: array, start: float, duration: float, base: float, amp: float) -> None:
    # Partiels quasi harmoniques et très décroissants : garde une couleur métal
    # sans les quatre pics inharmoniques agressifs de la première passe.
    for ratio, level, decay in [(1.0, 1.0, 2.0), (2.01, 0.30, 2.7), (3.98, 0.11, 3.4), (5.90, 0.035, 4.2)]:
        add_tone(buf, start, duration, base * ratio, amp * level, 0.0005, decay)


def add_warm_note(buf: array, start: float, duration: float, freq: float, amp: float,
                  attack: float = 0.012, decay: float = 2.2) -> None:
    """Timbre harmonique doux, sans partiels inharmoniques ni saturation."""
    for harmonic, level in ((1.0, 1.0), (2.0, 0.24), (3.0, 0.08)):
        add_tone(buf, start, duration, freq * harmonic, amp * level, attack, decay)


def weapon_pepin(rng: random.Random) -> array:
    b = mono(0.42)
    add_noise(b, rng, 0.0, 0.07, 1.4, lowpass=0.85, highpass=0.7)
    add_tone(b, 0.0, 0.16, 185.0, 0.8, sweep=-125.0, decay=2.4)
    add_metal(b, 0.018, 0.18, 890.0, 0.18)
    add_tone(b, 0.055, 0.30, 72.0, 0.24, attack=0.01, decay=3.0)
    return normalize(b)


def weapon_bourrasque(rng: random.Random) -> array:
    b = mono(0.34)
    add_noise(b, rng, 0.0, 0.17, 1.15, lowpass=0.32, highpass=0.55)
    add_tone(b, 0.0, 0.20, 310.0, 0.55, sweep=-245.0, decay=2.6)
    for offset in (0.0, 0.017, 0.034):
        add_tone(b, offset, 0.11, 980.0 - offset * 5000.0, 0.16, sweep=-400.0)
    add_tone(b, 0.06, 0.22, 96.0, 0.22, decay=3.2)
    return normalize(b)


def weapon_lenoir(rng: random.Random) -> array:
    b = mono(0.82)
    add_noise(b, rng, 0.0, 0.10, 1.5, lowpass=0.92, highpass=0.78)
    add_tone(b, 0.0, 0.34, 128.0, 1.0, sweep=-88.0, decay=2.0)
    add_metal(b, 0.025, 0.52, 740.0, 0.28)
    add_tone(b, 0.11, 0.62, 61.0, 0.44, attack=0.02, decay=2.8)
    add_tone(b, 0.18, 0.42, 1480.0, 0.08, attack=0.03, decay=3.0)
    return normalize(b)


def weapon_boucherie(rng: random.Random) -> array:
    b = mono(1.05)
    add_noise(b, rng, 0.0, 0.25, 1.55, lowpass=0.22)
    add_tone(b, 0.0, 0.55, 92.0, 1.35, sweep=-58.0, decay=1.8)
    add_tone(b, 0.015, 0.72, 47.0, 0.9, sweep=-18.0, decay=2.0)
    add_metal(b, 0.10, 0.46, 310.0, 0.20)
    add_noise(b, rng, 0.16, 0.72, 0.36, attack=0.03, decay=2.8, lowpass=0.08)
    return normalize(b, 0.96)


def short_effect(kind: str, rng: random.Random, variant: int = 0) -> array:
    durations = {"footstep": 0.24, "dash": 0.55, "reload_start": 0.20,
                 "reload_complete": 0.30, "switch": 0.42, "boon": 0.85,
                 "chest": 1.10, "wave_start": 1.20, "wave_clear": 1.35,
                 "boss": 1.55, "victory": 2.1, "defeat": 1.8,
                 "impact": 0.30, "weakspot": 0.42, "kill": 0.62,
                 "hurt": 0.48, "gold": 0.55, "souls": 0.90,
                 "ui_hover": 0.07, "ui_click": 0.12, "ui_tab": 0.20,
                 "ui_buy": 0.40, "ui_unlock": 0.65, "ui_denied": 0.28}
    b = mono(durations[kind])
    if kind == "impact":
        add_noise(b, rng, 0.0, 0.16, 0.86, lowpass=0.18)
        add_tone(b, 0.0, 0.24, 126.0, 0.48, sweep=-74.0, decay=2.6)
        add_metal(b, 0.012, 0.20, 360.0, 0.10)
    elif kind == "weakspot":
        add_metal(b, 0.0, 0.40, 1260.0, 0.38)
        add_tone(b, 0.015, 0.30, 2520.0, 0.12, decay=3.5)
    elif kind == "kill":
        add_tone(b, 0.0, 0.54, 98.0, 0.72, sweep=-48.0, decay=2.2)
        add_noise(b, rng, 0.0, 0.26, 0.62, lowpass=0.10)
        add_metal(b, 0.06, 0.40, 246.0, 0.14)
    elif kind == "hurt":
        add_tone(b, 0.0, 0.38, 72.0, 0.74, sweep=-28.0, decay=2.0)
        add_noise(b, rng, 0.0, 0.22, 0.55, lowpass=0.06)
    elif kind in ("gold", "souls"):
        notes = (1046.5, 1318.5) if kind == "gold" else (523.25, 783.99, 1174.66)
        for i, f in enumerate(notes):
            add_metal(b, i * 0.10, len(b) / SR - i * 0.10, f, 0.24)
        if kind == "souls":
            add_tone(b, 0.0, 0.82, 196.0, 0.18, attack=0.03, sweep=96.0, decay=2.0)
    elif kind == "footstep":
        # Talon puis semelle : deux impacts courts, masse basse non tonale et
        # micro-grains de pierre. Aucun tintement métallique systématique.
        weight = 0.72 + variant * 0.025
        add_noise(b, rng, 0.0, 0.085, weight, lowpass=0.045 + variant * 0.004)
        add_noise(b, rng, 0.055, 0.105, 0.46, attack=0.004, decay=2.8,
                  lowpass=0.12 + variant * 0.006)
        add_tone(b, 0.0, 0.11, 58.0 + variant * 1.7, 0.18, sweep=-18.0, decay=3.2)
        for grain in range(3 + variant % 3):
            at = 0.075 + grain * (0.012 + variant * 0.0007)
            add_noise(b, rng, at, 0.035, 0.10, lowpass=0.24)
    elif kind == "dash":
        add_noise(b, rng, 0.0, 0.46, 0.9, attack=0.015, decay=1.7, lowpass=0.45, highpass=0.6)
        add_tone(b, 0.0, 0.38, 170.0, 0.35, attack=0.015, sweep=620.0, decay=1.5)
    elif kind.startswith("reload"):
        add_metal(b, 0.0, 0.18, 630.0 if kind.endswith("start") else 810.0, 0.38)
        add_noise(b, rng, 0.0, 0.06, 0.25, lowpass=0.8)
        if kind.endswith("complete"):
            add_tone(b, 0.07, 0.20, 1320.0, 0.17, decay=3.5)
    elif kind == "switch":
        add_noise(b, rng, 0.0, 0.30, 0.48, attack=0.01, decay=2.0, lowpass=0.55, highpass=0.72)
        add_metal(b, 0.11, 0.28, 520.0, 0.16)
    elif kind in ("boon", "wave_clear", "victory"):
        notes = [440.0, 554.37, 659.25, 880.0] if kind != "victory" else [220.0, 329.63, 440.0, 659.25, 880.0]
        spacing = 0.12 if kind == "boon" else 0.22
        for i, f in enumerate(notes):
            add_metal(b, i * spacing, min(0.75, len(b) / SR - i * spacing), f, 0.22)
    elif kind == "chest":
        add_metal(b, 0.0, 0.45, 180.0, 0.28)
        add_noise(b, rng, 0.08, 0.52, 0.40, lowpass=0.12)
        for i, f in enumerate((523.25, 659.25, 783.99)):
            add_tone(b, 0.42 + i * 0.11, 0.42, f, 0.18, decay=2.8)
    elif kind == "wave_start":
        add_tone(b, 0.0, 1.1, 74.0, 0.65, attack=0.04, sweep=36.0, decay=1.5)
        add_metal(b, 0.18, 0.82, 146.0, 0.22)
        add_noise(b, rng, 0.0, 0.72, 0.32, attack=0.04, decay=1.7, lowpass=0.15)
    elif kind == "boss":
        for at in (0.0, 0.33, 0.66):
            add_tone(b, at, 0.72, 48.0, 0.72, attack=0.01, decay=2.0)
            add_metal(b, at, 0.60, 118.0, 0.20)
        add_noise(b, rng, 0.0, 1.35, 0.34, attack=0.1, decay=1.3, lowpass=0.07)
    elif kind == "defeat":
        for i, f in enumerate((293.66, 246.94, 196.0, 146.83)):
            add_tone(b, i * 0.30, 0.72, f, 0.28, attack=0.015, decay=2.0)
        add_tone(b, 0.65, 1.1, 49.0, 0.38, attack=0.08, decay=1.8)
    # ── Famille UI (story-678) — la plus discrète du pack : timbres chauds,
    # métal en pincée seulement, jamais de burst de bruit. Ces sons se répètent
    # des centaines de fois par session : la retenue EST la spécification.
    elif kind == "ui_hover":
        add_noise(b, rng, 0.0, 0.05, 0.30, decay=3.5, lowpass=0.10)
        add_warm_note(b, 0.0, 0.06, 660.0, 0.10, attack=0.002, decay=4.0)
    elif kind == "ui_click":
        add_noise(b, rng, 0.0, 0.04, 0.45, decay=3.0, lowpass=0.25)
        add_warm_note(b, 0.0, 0.10, 392.0, 0.28, attack=0.001, decay=3.2)
    elif kind == "ui_tab":
        add_noise(b, rng, 0.0, 0.16, 0.35, attack=0.008, decay=2.2,
                  lowpass=0.5, highpass=0.4)
        add_warm_note(b, 0.04, 0.14, 523.25, 0.14, decay=3.0)
    elif kind == "ui_buy":
        add_warm_note(b, 0.0, 0.26, 523.25, 0.26, decay=2.6)
        add_warm_note(b, 0.10, 0.28, 659.25, 0.24, decay=2.6)
        add_metal(b, 0.12, 0.25, 1046.5, 0.06)
    elif kind == "ui_unlock":
        for i, f in enumerate((392.0, 523.25, 659.25)):
            add_warm_note(b, i * 0.11, 0.34, f, 0.24, decay=2.4)
        add_metal(b, 0.30, 0.30, 783.99, 0.05)
    elif kind == "ui_denied":
        add_warm_note(b, 0.0, 0.16, 196.0, 0.30, attack=0.004, decay=2.0)
        add_warm_note(b, 0.09, 0.18, 174.61, 0.28, attack=0.004, decay=2.0)
    return normalize(b, 0.88)


def forge_ambience(rng: random.Random, duration: float = 24.0) -> tuple[array, array]:
    left, right = stereo(duration)
    n = len(left)
    phases = [rng.uniform(0.0, TAU) for _ in range(8)]
    for i in range(n):
        t = i / SR
        rumble = math.sin(TAU * 43.0 * t) * 0.055 + math.sin(TAU * 67.0 * t) * 0.025
        # Somme de sinusoïdes à nombre entier de cycles : texture organique mais
        # exactement périodique, donc pas de clic ni de creux à la couture.
        air_l = sum(math.sin(TAU * (k + 1) * t / duration + phases[k]) for k in range(4)) / 4.0
        air_r = sum(math.sin(TAU * (k + 1) * t / duration + phases[k + 4]) for k in range(4)) / 4.0
        breathe = 0.65 + 0.35 * math.sin(TAU * t / 8.0)
        left[i] = rumble + air_l * 0.045 * breathe
        right[i] = rumble * 0.92 + air_r * 0.045 * breathe
    for at, pan in [(2.2, -0.6), (5.7, 0.5), (9.1, -0.2), (13.8, 0.7), (18.0, -0.7), (21.4, 0.25)]:
        tmp = mono(1.6)
        add_metal(tmp, 0.0, 1.5, 176.0 + at * 2.0, 0.18)
        add_noise(tmp, rng, 0.02, 0.32, 0.18, lowpass=0.5)
        begin = int(at * SR)
        for j, value in enumerate(tmp[: max(0, n - begin)]):
            left[begin + j] += value * (1.0 - pan) * 0.5
            right[begin + j] += value * (1.0 + pan) * 0.5
    joined = normalize(array("f", list(left) + list(right)), 0.72)
    return joined[:n], joined[n:]


def forge_music(rng: random.Random, duration: float = 60.0) -> tuple[array, array]:
    left, right = stereo(duration)
    n = len(left)
    bpm = 96.0
    beat = 60.0 / bpm
    bar_len = beat * 4.0
    # D mineur naturel. A–B–A' : hook reconnaissable, contraste, retour varié.
    chords = [
        (146.83, 174.61, 220.00),  # Dm
        (116.54, 146.83, 174.61),  # Bb
        (130.81, 164.81, 196.00),  # F
        (130.81, 164.81, 196.00),  # F/A, respiration
        (146.83, 174.61, 220.00),  # Dm
        (98.00, 116.54, 146.83),   # Gm
        (110.00, 138.59, 164.81),  # A tension douce
        (146.83, 174.61, 220.00),  # résolution
    ]
    hook = [293.66, 349.23, 440.00, 392.00, 349.23, 329.63, 293.66, None]
    response = [220.00, 261.63, 293.66, 349.23, 329.63, 293.66, 261.63, None]
    bars = 24
    for bar in range(bars):
        bar_at = bar * bar_len
        section = bar // 8
        intensity = (0.56, 0.78, 0.68)[section]
        chord = chords[bar % 8]
        # Pad doux : attaque lente et grand espace entre les registres.
        for note_index, freq in enumerate(chord):
            tmp = mono(bar_len * 0.94)
            add_warm_note(tmp, 0.0, bar_len * 0.90, freq, 0.055 * intensity,
                          attack=0.18, decay=1.3)
            pan = (-0.32, 0.0, 0.32)[note_index]
            begin = int(bar_at * SR)
            for j, value in enumerate(tmp[: max(0, n - begin)]):
                left[begin + j] += value * (1.0 - pan) * 0.5
                right[begin + j] += value * (1.0 + pan) * 0.5
        # Percussion de forge rare : le silence entre les coups est structurel.
        if section > 0 or bar % 2 == 0:
            for pulse in ((0.0, 2.5) if section == 1 else (0.0,)):
                tmp = mono(0.34)
                add_tone(tmp, 0.0, 0.30, 64.0, 0.20 * intensity, sweep=-22.0, decay=2.8)
                add_noise(tmp, rng, 0.0, 0.055, 0.11 * intensity, lowpass=0.055)
                begin = int((bar_at + pulse * beat) * SR)
                for j, value in enumerate(tmp[: max(0, n - begin)]):
                    left[begin + j] += value
                    right[begin + j] += value
        phrase = response if section == 1 else hook
        # Un motif toutes les deux mesures seulement : anticipation puis réponse.
        if bar % 2 == 0:
            for step, freq in enumerate(phrase):
                if freq is None:
                    continue
                at = bar_at + step * beat * 0.5
                tmp = mono(beat * 0.82)
                add_warm_note(tmp, 0.0, beat * 0.78, freq, 0.075 * intensity)
                pan = -0.18 if step % 2 == 0 else 0.18
                begin = int(at * SR)
                for j, value in enumerate(tmp[: max(0, n - begin)]):
                    left[begin + j] += value * (1.0 - pan) * 0.5
                    right[begin + j] += value * (1.0 + pan) * 0.5
    joined = normalize(array("f", list(left) + list(right)), 0.70)
    return joined[:n], joined[n:]


def write_wav(path: Path, channels: tuple[array, ...]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    length = min(len(ch) for ch in channels)
    pcm = bytearray()
    for i in range(length):
        for channel in channels:
            pcm.extend(struct.pack("<h", int(max(-1.0, min(1.0, channel[i])) * 32767)))
    with wave.open(str(path), "wb") as out:
        out.setnchannels(len(channels))
        out.setsampwidth(2)
        out.setframerate(SR)
        out.writeframes(pcm)


def encode_ogg(wav_path: Path, ogg_path: Path) -> None:
    ogg_path.parent.mkdir(parents=True, exist_ok=True)
    relative = ogg_path.as_posix()
    if "/music/" in relative:
        target, lowpass, true_peak = "-24", "8000", "-4"
    elif "/ambience/" in relative:
        target, lowpass, true_peak = "-30", "6500", "-8"
    elif "/weapons/" in relative:
        target, lowpass, true_peak = "-22", "8500", "-6"
    elif "/ui/" in relative:
        # La famille la plus discrète : sous tout le reste du mix, aigus adoucis.
        # TP borné à -9 : loudnorm refuse en dessous (plage [-9, 0]).
        target, lowpass, true_peak = "-28", "6000", "-9"
    else:
        target, lowpass, true_peak = "-25", "7000", "-8"
    subprocess.run([
        "ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-i", str(wav_path),
        "-af", f"highpass=f=28,lowpass=f={lowpass},loudnorm=I={target}:TP={true_peak}:LRA=12",
        "-ar", str(SR), "-c:a", "libvorbis", "-q:a", "6", "-metadata", "artist=Forgia",
        "-metadata", "copyright=Original procedural asset - Forgia project",
        str(ogg_path),
    ], check=True)


def encode_footstep_source(source: Path, ogg_path: Path) -> None:
    """Prépare une prise Foley Kenney sans la compresser jusqu'au plafond."""
    subprocess.run([
        "ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-i", str(source),
        "-af", "highpass=f=45,lowpass=f=5200,volume=-12dB",
        "-ar", str(SR), "-ac", "1", "-c:a", "libvorbis", "-q:a", "6",
        "-metadata", "artist=Kenney Vleugels / Forgia edit",
        "-metadata", "copyright=CC0 1.0 - processed for Forgia",
        str(ogg_path),
    ], check=True)


def encode_weapon_source(source: Path, ogg_path: Path, gain_db: float,
                         highpass: int, lowpass: int) -> None:
    """Garde la dynamique d'une vraie détonation et crée seulement sa place au mix."""
    subprocess.run([
        "ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-i", str(source),
        "-af", f"highpass=f={highpass},lowpass=f={lowpass},volume={gain_db}dB,alimiter=limit=0.50:attack=2:release=80:level=false",
        "-ar", str(SR), "-ac", "1", "-c:a", "libvorbis", "-q:a", "7",
        "-metadata", "artist=Free Firearm Sound Library / Forgia edit",
        "-metadata", "copyright=CC0 1.0 - processed for Forgia",
        str(ogg_path),
    ], check=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    root = args.root.resolve()
    masters = root / "target" / "audio-masters"
    output = root / "assets" / "audio" / "forgia_original"
    if not shutil.which("ffmpeg"):
        raise SystemExit("ffmpeg est requis dans PATH")
    rng = random.Random(SEED)
    # RNG séparé pour la famille UI (story-678) : insérer des consommateurs dans
    # le flux principal décalerait la graine de l'ambiance et de la musique déjà
    # validées — elles doivent régénérer À L'IDENTIQUE.
    ui_rng = random.Random(SEED ^ 0x0678)
    assets: dict[str, tuple[array, ...]] = {
        "movement/dash_ember": (short_effect("dash", rng),),
        "movement/reload_start": (short_effect("reload_start", rng),),
        "movement/reload_complete": (short_effect("reload_complete", rng),),
        "movement/weapon_switch": (short_effect("switch", rng),),
        "events/boon_forged": (short_effect("boon", rng),),
        "events/chest_open": (short_effect("chest", rng),),
        "events/wave_start": (short_effect("wave_start", rng),),
        "events/wave_clear": (short_effect("wave_clear", rng),),
        "events/boss_enrage": (short_effect("boss", rng),),
        "events/victory": (short_effect("victory", rng),),
        "events/defeat": (short_effect("defeat", rng),),
        "combat/impact_forge": (short_effect("impact", rng),),
        "combat/weakspot_chime": (short_effect("weakspot", rng),),
        "combat/kill_stamp": (short_effect("kill", rng),),
        "combat/player_hurt": (short_effect("hurt", rng),),
        "pickups/gold_spark": (short_effect("gold", rng),),
        "pickups/soul_echo": (short_effect("souls", rng),),
        "ui/ui_hover": (short_effect("ui_hover", ui_rng),),
        "ui/ui_click": (short_effect("ui_click", ui_rng),),
        "ui/ui_tab": (short_effect("ui_tab", ui_rng),),
        "ui/ui_buy": (short_effect("ui_buy", ui_rng),),
        "ui/ui_unlock": (short_effect("ui_unlock", ui_rng),),
        "ui/ui_denied": (short_effect("ui_denied", ui_rng),),
    }
    assets["ambience/forge_heart_loop"] = forge_ambience(rng)
    assets["music/forged_destiny_loop"] = forge_music(rng)
    for relative, channels in assets.items():
        wav_path = masters / f"{relative}.wav"
        ogg_path = output / f"{relative}.ogg"
        write_wav(wav_path, channels)
        encode_ogg(wav_path, ogg_path)
        print(f"generated {ogg_path.relative_to(root)}")
    footstep_sources = root / "assets" / "audio" / "sources" / "kenney_rpg"
    for i in range(6):
        source = footstep_sources / f"footstep{i:02}.ogg"
        ogg_path = output / "footsteps" / f"forge_stone_{i + 1:02}.ogg"
        encode_footstep_source(source, ogg_path)
        print(f"processed {ogg_path.relative_to(root)} from {source.name} (CC0)")
    firearm_sources = root / "assets" / "audio" / "sources" / "free_firearm"
    weapon_edits = [
        ("pepin_ruger.flac", "pepin_forge.ogg", -8.0, 70, 8000),
        ("bourrasque_m45.flac", "bourrasque_gale.ogg", -10.0, 80, 7500),
        ("lenoir_mosin.flac", "lenoir_royal.ogg", -6.0, 45, 9000),
        ("boucherie_mossberg.flac", "boucherie_furnace.ogg", -5.0, 35, 8500),
    ]
    for source_name, output_name, gain, highpass, lowpass in weapon_edits:
        source = firearm_sources / source_name
        ogg_path = output / "weapons" / output_name
        encode_weapon_source(source, ogg_path, gain, highpass, lowpass)
        print(f"processed {ogg_path.relative_to(root)} from {source.name} (CC0)")


if __name__ == "__main__":
    main()
