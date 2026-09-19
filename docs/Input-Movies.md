# Input Movies (record human play, replay to video)

Minimal workflow for playthrough videos: play once, save inputs, replay later.

## Record (standalone, windowed)

```powershell
native32-emu --record-input runs\clear.nmov tmp\native32_game\EACT\EBBLADE.smf
```

Inputs are written as plain text (`.nmov`) or JSON (`.json` extension). The file
is flushed every 300 frames and finalized on exit.

## Record (RetroArch core)

- Core Options → **Record input movie (text)** → enabled  
  → `<system>/Native32Emu/inputs/<content>-<timestamp>.nmov`
- Or set env `NATIVE32_RECORD_INPUT=D:\runs\clear.nmov` before launch  
Unload content / quit to finalize.

## Replay

```powershell
# In-emulator (window): not required — use headless replay for video
powershell -File scripts\replay-movie.ps1 -Movie runs\clear.nmov
```

`--replay` loads the movie, injects keys each frame, dumps PNGs + WAV audio.
The script encodes `replay.mp4` (H.264 + AAC) when FFmpeg is on PATH.

Direct CLI:

```powershell
native32-emu --replay runs\clear.nmov `
  --dump-frames out\frames --dump-every 2 --record-audio out\audio.wav `
  path\to\game.smf
```

## Movie format

```text
# Native32 input movie v1
game: path/to/game.smf
fps: 30
auto_skip_cutscenes: true
# frame keys... [hold N]   keys: L R U D A B or 0xNNNN
0 hold 30
30 A hold 5
80 R A hold 15
```

## CLI flags

| Flag | Meaning |
|---|---|
| `--record-input <path>` | Record windowed play to a movie |
| `--replay <movie>` | Headless replay (no window) |
| `--dump-frames <dir>` | PNG dump during replay |
| `--dump-every <N>` | Dump every N frames (default 2) |
| `--record-audio <wav>` | Mixed audio WAV during replay |
| `--max-frames <N>` | Replay safety cap |
