# please

Describe what you want in plain words; Claude writes the shell command; you confirm; it runs.

    please compress the video in this folder

```
  ffmpeg -i holiday.mov -c:v libx264 -crf 28 ... holiday_compressed.mp4

Run it? [Y/n/e=edit]
```

Enter runs it, `n` cancels, `e` lets you type a replacement.

## Requirements

- Claude Code CLI (`claude`) installed and logged in: https://claude.com/claude-code
- Rust (only to build): https://rustup.rs

## Build & install (macOS, Linux, Windows)

    cargo install --path .

That puts `please` in `~/.cargo/bin` (already on your PATH if you installed Rust with rustup).

## Notes

- Runs commands with your `$SHELL` on macOS/Linux and PowerShell on Windows.
- Default model is Haiku (fast). Use a stronger one with `PLEASE_MODEL=sonnet`.
- Quote requests that contain apostrophes: `please "resize this folder's images"`.
