#!/usr/bin/env python3
"""Reference labels for the key/BPM accuracy test (development only).

Nothing in the app imports or runs this script. It uses Essentia (AGPL), which is fine
because Essentia never ships with the app nor becomes a dependency of the repository:
install it in a virtual environment OUTSIDE the repository, e.g.

    python3 -m venv /some/scratch/venv
    /some/scratch/venv/bin/python -I -m pip install essentia
    /some/scratch/venv/bin/python -I scripts/reference_labels.py --init <pack> labels.csv
    /some/scratch/venv/bin/python -I scripts/reference_labels.py <pack> labels.csv labels.csv

Two steps:

  --init <pack> <csv-out>
      Writes the answer key that the folder and file names alone give (DAYDREAMS layout):
      kind from the folder, BPM from "(NNbpm)" in the name, `source=name`. Melodic
      one-shots get an empty kind (their kind comes from the second step).

  <pack> <csv-in> <csv-out>
      Fills every empty label with a reference value and marks it `source=reference`:
        * melodic loops and phrases: KeyExtractor, profile "edma" (the Krumhansl and
          Temperley answers are printed to stderr so disagreements among profiles show);
        * notes: median of PitchYinFFT over the clear frames (confidence >= 0.8);
        * chords: the most frequent ChordsDetection label (Essentia only knows major and
          minor triads, so 7th chords come out as their triad).
      Melodic one-shots with an empty kind get a proposed kind (chord if the name says so,
      phrase if two or more different notes are held, chord if three or more pitch classes
      stay strong, else note), also `source=reference`.

Reference labels are unverified until the author checks them by ear (`source=author`).
Paths in the CSV are relative to the pack folder, with "/" separators.
"""

import csv
import os
import re
import sys
from collections import Counter

FLATS = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"]
SHARP_TO_INDEX = {n: i for i, n in enumerate(["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"])}
SHARP_TO_INDEX.update({n: i for i, n in enumerate(FLATS)})
RATE = 44100
AUDIO_EXT = (".wav", ".aif", ".aiff", ".flac", ".mp3", ".ogg")
BPM_RE = re.compile(r"\((\d+(?:\.\d+)?)\s*bpm\)", re.IGNORECASE)
HEADER = ["path", "kind", "label", "bpm", "source"]


def flat_name(name):
    return FLATS[SHARP_TO_INDEX[name]]


def audio_files(root):
    for folder, dirs, files in os.walk(root):
        dirs.sort()
        for f in sorted(files):
            if f.lower().endswith(AUDIO_EXT) and not f.startswith("."):
                yield os.path.relpath(os.path.join(folder, f), root).replace(os.sep, "/")


def kind_from_folder(rel):
    top = rel.split("/")[0]
    name = rel.split("/")[-1]
    if top in ("Drum Breaks", "Percussion Loops"):
        return "drumLoop"
    if top == "Sample Loops":
        return "melodicLoop"
    if top == "One Shots":
        # Short tambourine loops filed among the snares carry a BPM: they are loops.
        return "drumLoop" if BPM_RE.search(name) else "unpitched"
    return ""  # One Shots (Melodic): decided by the second step


def init(root, out_path):
    rows = []
    for rel in audio_files(root):
        kind = kind_from_folder(rel)
        m = BPM_RE.search(rel.split("/")[-1])
        bpm = m.group(1) if m and kind in ("drumLoop", "melodicLoop") else ""
        rows.append({"path": rel, "kind": kind, "label": "", "bpm": bpm, "source": "name"})
    write(out_path, rows)


def write(path, rows):
    with open(path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=HEADER, lineterminator="\n")
        w.writeheader()
        for r in rows:
            w.writerow({k: r.get(k, "") for k in HEADER})


def load(path):
    import essentia.standard as es

    return es.MonoLoader(filename=path, sampleRate=RATE)()


def key_of(audio, profile):
    import essentia.standard as es

    key, scale, strength = es.KeyExtractor(profileType=profile, sampleRate=RATE)(audio)
    return flat_name(key) + ("m" if scale == "minor" else ""), float(strength)


def pitch_track(audio):
    """(Hz, confidence) per 2048-sample frame, hop 512, from PitchYinFFT.

    PitchYinProbabilistic (pYIN) was the first choice, but in the Essentia wheel tested
    (2.1b6.dev1389, macOS, Python 3.9) it returns the same garbage pitch for every frame,
    so the FFT version of YIN is used instead."""
    import essentia.standard as es

    window, spectrum = es.Windowing(type="hann"), es.Spectrum()
    yin = es.PitchYinFFT(sampleRate=RATE, frameSize=2048)
    track = []
    for frame in es.FrameGenerator(audio, frameSize=2048, hopSize=512, startFromZero=True):
        if float((frame ** 2).mean()) < 1e-7:
            track.append((0.0, 0.0))
            continue
        hz, confidence = yin(spectrum(window(frame)))
        track.append((float(hz), float(confidence)))
    return track


def midi(hz):
    import math

    return 69.0 + 12.0 * math.log2(hz / 440.0)


def note_of(track):
    voiced = sorted(midi(p) for p, v in track if p > 0 and v >= 0.8)
    if not voiced:
        return None
    return FLATS[int(round(voiced[len(voiced) // 2])) % 12]


def held_notes(track, min_frames=5):
    """Distinct rounded notes held for at least `min_frames` clear frames in a row (~115 ms); notes a
    semitone apart count once, so a sample tuned between two notes is not a phrase."""
    notes, run_note, run = set(), None, 0
    for p, v in track:
        n = int(round(midi(p))) if p > 0 and v >= 0.8 else None
        if n is not None and n == run_note:
            run += 1
        else:
            run_note, run = n, 1
        # A note within a semitone of one already held is the same detuned note.
        if n is not None and run == min_frames and all(abs(n - m) > 1 for m in notes):
            notes.add(n)
    return notes


def hpcp_frames(audio):
    import essentia.standard as es

    frame, hop = 4096, 2048
    window = es.Windowing(type="blackmanharris62")
    spectrum = es.Spectrum()
    peaks = es.SpectralPeaks(orderBy="magnitude", magnitudeThreshold=1e-5, minFrequency=40, maxFrequency=5000, maxPeaks=60, sampleRate=RATE)
    hpcp = es.HPCP(size=12, referenceFrequency=440, harmonics=8, bandPreset=True, minFrequency=40, maxFrequency=5000, weightType="cosine", nonLinear=False, windowSize=1.0, sampleRate=RATE)
    out, energy = [], []
    for f in es.FrameGenerator(audio, frameSize=frame, hopSize=hop, startFromZero=True):
        s = spectrum(window(f))
        freqs, mags = peaks(s)
        out.append(hpcp(freqs, mags))
        energy.append(float((f ** 2).sum()))
    return out, energy, hop


def loud_frames(frames, energy):
    peak = max(energy) if energy else 0.0
    return [h for h, e in zip(frames, energy) if peak > 0 and e >= peak * 0.01]


def chord_of(audio):
    import numpy as np
    import essentia.standard as es

    frames, energy, hop = hpcp_frames(audio)
    keep = loud_frames(frames, energy)
    if not keep:
        return None
    chords, _ = es.ChordsDetection(hopSize=hop, sampleRate=RATE, windowSize=1.0)(np.array(keep, dtype=np.float32))
    label = Counter(chords).most_common(1)[0][0]
    minor = label.endswith("m")
    return flat_name(label[:-1] if minor else label) + ("m" if minor else "")


def strong_classes(audio):
    """Pitch classes at half the strongest class or more in the mean HPCP of the loud frames."""
    frames, energy, _ = hpcp_frames(audio)
    keep = loud_frames(frames, energy)
    if not keep:
        return 0
    mean = [sum(f[i] for f in keep) / len(keep) for i in range(12)]
    top = max(mean)
    return sum(1 for v in mean if top > 0 and v >= 0.5 * top)


def propose_kind(rel, audio, track):
    name = rel.split("/")[-1].lower()
    if "chord" in name:
        return "chord"
    if len(held_notes(track)) >= 2:
        return "phrase"
    if strong_classes(audio) >= 3:
        return "chord"
    return "note"


def fill(root, in_path, out_path):
    with open(in_path, newline="") as f:
        rows = list(csv.DictReader(f))
    for r in rows:
        r.setdefault("source", "")
        rel = r["path"]
        if r["label"] and r["kind"]:
            continue
        top = rel.split("/")[0]
        if r["kind"] not in ("melodicLoop", "phrase", "note", "chord") and not (r["kind"] == "" and top == "One Shots (Melodic)"):
            continue
        audio = load(os.path.join(root, rel))
        track = pitch_track(audio)
        if not r["kind"]:
            r["kind"] = propose_kind(rel, audio, track)
            r["source"] = "reference"
        if r["kind"] in ("melodicLoop", "phrase"):
            label, strength = key_of(audio, "edma")
            others = {p: key_of(audio, p)[0] for p in ("krumhansl", "temperley", "bgate")}
            sys.stderr.write(f"{rel}: edma={label} ({strength:.2f}) " + " ".join(f"{p}={k}" for p, k in others.items()) + "\n")
        elif r["kind"] == "note":
            label = note_of(track) or ""
            sys.stderr.write(f"{rel}: note={label} held={sorted(held_notes(track))}\n")
        else:
            label = chord_of(audio) or ""
            sys.stderr.write(f"{rel}: chord={label} strong_classes={strong_classes(audio)}\n")
        r["label"] = label
        r["source"] = "reference"
    write(out_path, rows)


def main(argv):
    if len(argv) == 3 and argv[0] == "--init":
        init(argv[1], argv[2])
    elif len(argv) == 3:
        fill(*argv)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
