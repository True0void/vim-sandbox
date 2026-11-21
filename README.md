# vim-sandbox

A minimalist Vim-like editor with artistic animated backgrounds. Written in Rust with ratatui.

## Features

- **Vim-like editing** — Normal, Insert, Visual, Command modes with motions, operators, registers, undo/redo
- **10 artistic themes** — Procedural ASCII animations running at 60fps in the background
- **True transparency** — Works with compositors (picom, hyprland, etc.) via OSC 11 sequences
- **File operations** — `:e`, `:w`, `:q`, `:wq`, search `/`, tabs completion for themes
- **Zero dependencies on emoji** — Pure ASCII/Unicode box-drawing characters

## Themes

| Name | Description |
|------|-------------|
| `spider-lily` | 彼岸花 — Red spider lilies blooming/wilting, petals drifting (Tokyo Ghoul aesthetic) |
| `sakura-night` | 夜桜 — Cherry blossoms at night under moonlight |
| `abyssal-bloom` | 深海発光 — Bioluminescent deep-sea creatures |
| `firefly-dusk` | 蛍の黄昏 — Fireflies dancing, grass swaying |
| `aurora-void` | 極光の虚空 — Cosmic aurora curtains |
| `koi-dreams` | 錦鯉の夢 — Koi fish, water lilies, ripples |
| `bamboo-whisper` | 竹の囁き — Bamboo forest swaying |
| `nebula-heart` | 星雲の心臓 — Pulsing nebula cores |
| `rain-glass` | 雨のガラス — Rain drops on glass |
| `willow-tears` | 垂柳の涙 — Weeping willow branches |

## Installation

```bash
# Requires Rust 1.70+
git clone https://github.com/<your-username>/vim-sandbox
cd vim-sandbox
cargo build --release
./target/release/vim-sandbox [file]
```

## Usage

```bash
vim-sandbox                    # New buffer
vim-sandbox main.rs            # Open file
```

### Key Bindings

| Mode | Keys | Action |
|------|------|--------|
| **Normal** | `h j k l` | Move cursor |
| | `w b e` `0 ^ $` `gg G` | Word/line motions |
| | `i a o O I A` | Enter Insert mode |
| | `dd yy cc` `p P` | Delete/Yank/Change + Paste |
| | `u` `Ctrl+r` | Undo / Redo |
| | `v V` | Visual mode |
| | `:` | Command mode |
| **Insert** | `Esc` | Return to Normal |
| **Command** | `:w` `:w file` | Save / Save as |
| | `:q` `:q!` | Quit / Force quit |
| | `:wq` `:x` | Save and quit |
| | `:e file` `:e! file` | Open / Reopen file |
| | `/pattern` `?pattern` | Search forward/backward |
| | `:theme name` | Change theme (Tab completion) |
| | `:themes` | List all themes |
| | `:themenext` `:themeprev` | Cycle themes |

### Theme Commands

```vim
:themes                      " List available themes
:theme spider-lily           " Set theme
:theme sak<tab>              " Tab completion → sakura-night
:colorscheme aurora-void     " Alias
:themenext / :themeprev      " Next / Previous theme
```

## Architecture

```
src/
├── main.rs                 # Entry point, event loop, terminal setup
├── terminal_guard.rs       # RAII terminal cleanup
├── editor/
│   └── core.rs             # Editor logic: modes, motions, operators, registers, undo
├── ui/
│   ├── background.rs       # Theme registry, animation loop
│   ├── render.rs           # Ratatui rendering (transparent layers)
│   ├── themes.rs           # 10 procedural ASCII themes
│   └── transparency.rs     # OSC 11 true transparency
└── sandbox/                # Hot-reload infrastructure (unused)
```

## Transparency

True transparency via OSC 11 escape sequences. Requires:
- Terminal: kitty, alacritty, wezterm, ghostty, iTerm2, or any with compositor
- Compositor: picom, hyprland, sway, wayfire, kwin, mutter

## Requirements

- Rust 1.70+
- Linux/macOS (Windows untested)
- Terminal with 24-bit color support

## License

MIT

## Author

**chef** — [GitHub](https://github.com/chef)