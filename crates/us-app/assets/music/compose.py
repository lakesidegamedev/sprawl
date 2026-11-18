"""Render the original Urban Sprawl score with NumPy and FFmpeg.

Run this file to rebuild the two stereo Vorbis loops beside it. Patterns,
instruments, and the modulation are synthesized here without samples.
"""

from pathlib import Path
import subprocess

import numpy as np

RATE = 24_000
BAR = 60 / 76 * 4
OUT = Path(__file__).resolve().parent
BRIDGE = [(-2, 2, 5, 9), (0, 4, 7, 11), (5, 9, 12, 16), (7, 10, 14, 17)]
DAY = [(0, 4, 7, 11), (7, 9, 14, 19), (9, 12, 16, 19), (5, 9, 12, 16)]
DUSK = [(0, 3, 7, 10), (8, 12, 15, 19), (3, 7, 10, 14), (10, 14, 17, 21)]
PHRASES = [
    [(0.5, 11), (1.5, 9), (2.25, 7)],
    [(0.25, 4), (1.25, 7), (3.0, 14)],
    [(0.75, 16), (2, 12), (2.75, 9)],
    [(1, 7), (2, 4), (3.5, 2)],
]


def voice(length, midi, pad):
    t = np.arange(int(length * RATE), dtype=np.float32) / RATE
    hz = 440 * 2 ** ((midi - 69) / 12)
    phase = 2 * np.pi * hz * t
    if pad:
        envelope = np.minimum(1, t / 0.62) * np.minimum(1, (length - t) / 1.0)
        wave = 0.55 * np.sin(phase) + 0.14 * np.sin(phase * 1.002)
        wave += 0.09 * np.sin(phase * 2.002 + np.sin(t * 0.72) * 0.4)
        return envelope * wave
    envelope = np.minimum(1, t / 0.018) * np.exp(-t * 2.3)
    envelope *= np.minimum(1, (length - t) / 0.14)
    wave = np.sin(phase + np.sin(phase * 2.0) * np.exp(-t * 6.0) * 0.6)
    wave += 0.21 * np.sin(phase * 2.002) * np.exp(-t * 3.0)
    return envelope * wave


def render(name, root, chords, dark):
    frames = int(BAR * 24 * RATE)
    mix = np.zeros((frames, 2), dtype=np.float32)

    def play(time, duration, midi, volume, pan=0.0, pad=False):
        audio = voice(duration, midi, pad) * volume
        idx = (int(time * RATE) + np.arange(len(audio))) % frames
        mix[idx, 0] += audio * np.sqrt((1 - pan) * 0.5)
        mix[idx, 1] += audio * np.sqrt((1 + pan) * 0.5)

    for bar in range(24):
        start = bar * BAR
        progression = BRIDGE if 12 <= bar < 16 else chords
        chord = progression[bar % 4]
        play(start, BAR * 0.95, root + chord[0] - 12, 0.17, pad=True)
        for lane, note in enumerate(chord):
            offset = lane * 0.055
            play(start + offset, BAR * 1.23, root + note, 0.105, (-0.48 + lane * 0.32), True)
        for j, beat in enumerate([0.0, 0.75, 1.5, 2.25, 3.5] if not dark else [0.0, 1.5, 2.75]):
            note = chord[(j + bar) % len(chord)]
            play(start + beat * BAR / 4, 1.5, root + note + (12 if j % 3 == 1 else 0), 0.11, (-1) ** j * 0.34)
        if bar >= 4 and bar % 8 < 6:
            for beat, note in PHRASES[bar % 4]:
                pitch = note - int(dark and note in (4, 9, 16))
                play(start + beat * BAR / 4, 1.7, root + pitch + 12, 0.084, 0.28)
    reverberation = sum(np.roll(mix[:, ::-1], int(RATE * t), axis=0) * gain
                        for t, gain in [(0.19, 0.12), (0.29, 0.095), (0.47, 0.06), (0.71, 0.035)])
    result = np.clip(mix + reverberation, -0.95, 0.95)
    subprocess.run(
        ["ffmpeg", "-nostdin", "-loglevel", "error", "-y", "-f", "f32le",
         "-ar", str(RATE), "-ac", "2", "-i", "pipe:0", "-c:a", "libvorbis", "-q:a", "5",
         str(OUT / f"{name}.ogg")],
        input=result.astype("<f4").tobytes(), check=True,
    )


if __name__ == "__main__":
    render("morning-neighborhoods", 52, DAY, False)
    render("last-tram-home", 50, DUSK, True)
