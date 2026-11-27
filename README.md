# vim-sandbox

A minimalist Vim-like editor with artistic animated backgrounds. Written in Rust with ratatui.

## Demo

Watch the themes in action:

| Theme | Preview |
|-------|---------|
| spider-lily | ![spider-lily](https://github.com/user-attachments/assets/f8ea383d-9f23-4876-a57e-6acd8b73ecdb) |
| spider-lily  | ![sakura-night](https://github.com/user-attachments/assets/45e11f10-3a1f-4dbd-be00-549effcc1641) |
| spider-lily   | ![abyssal-bloom](https://github.com/user-attachments/assets/11a6c1eb-1417-4b39-a949-38f12f8bdc18) |

*Generate your own screenshots: `asciinema rec cast.cast && cargo run --release`, then convert with `asciinema2gif`. Place GIFs in `docs/screenshots/` and link them above.*

## Features

- **Vim-like editing** — Normal, Insert, Visual, Command modes with motions, operators, registers, undo/redo
- **10 animated themes** — Procedural braille-art backgrounds running at 60fps
- **True transparency** — Works with compositors (picom, hyprland, etc.) via OSC 11 sequences
- **File operations** — `:e`, `:w`, `:q`, `:wq`, search `/`, tabs completion for themes
- **Zero dependencies on emoji** — Pure ASCII/Unicode Braille characters

## Themes

| Name | Description |
|------|-------------|
| `spider-lily` | 彼岸花 — Red spider lilies blooming/wilting, petals drifting (Tokyo Ghoul aesthetic) |
| `sakura-night` | 夜桜 — Cherry blossoms drifting under a moonlight sky |
| `abyssal-bloom` | 深海発光 — Bioluminescent deep-sea creatures glowing in the dark |
| `firefly-dusk` | 蛍の黄昏 — Fireflies dancing, grass swaying at dusk |
| `aurora-void` | 極光の虚空 — Cosmic aurora curtains pulsing in deep space |
| `koi-dreams` | 錦鯉の夢 — Koi fish, water lilies, ripples in a moonlit pond |
| `bamboo-whisper` | 竹の囁き — Bamboo forest swaying in the breeze |
| `nebula-heart` | 星雲の心臓 — Pulsing nebula cores in deep space |
| `rain-glass` | 雨のガラス — Raindrops on glass with city lights behind |
| `willow-tears` | 垂柳の涙 — Weeping willow branches with falling droplets |

Switch themes at runtime: `:theme sakura-night`, `:themenext`, `:themeprev`, `:themes`.

## Installation

```bash
# Requires Rust 1.70+
git clone https://github.com/True0void/vim-sandbox
cd vim-sandbox
cargo build --release
./target/release/vim-sandbox
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
| | `D Y C S` | Linewise delete/yank/change/substitute |
| | `:` | Command mode |
| | `Ctrl+q` | Quit |
| | `Ctrl+s` | Save |
| **Insert** | `Esc` | Return to Normal |
| **Command** | `:w` `:w file` | Save / Save as |
| | `:q` `:q!` | Quit / Force quit |
| | `:wq` `:x` | Save and quit |
| | `:e file` `:e! file` | Open / Reopen file |
| | `/pattern` `?pattern` | Search forward/backward |
| | `n` `N` | Search next / previous |
| | `:theme name` | Change theme |
| | `:themes` | List all themes |
| | `:themenext` `:themeprev` | Cycle themes |
| | `:help` | Show help |

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
│   ├── themes/
│   │   ├── mod.rs          # Theme trait + registry (10 themes)
│   │   ├── spider_lily.rs  # Red spider lilies garden
│   │   ├── sakura_night.rs # Cherry blossoms under moonlight
│   │   ├── abyssal_bloom.rs# Bioluminescent deep-sea creatures
│   │   ├── firefly_dusk.rs # Fireflies at dusk
│   │   ├── aurora_void.rs  # Cosmic aurora curtains
│   │   ├── koi_dreams.rs   # Koi fish in moonlit pond
│   │   ├── bamboo_whisper.rs# Bamboo forest in breeze
│   │   ├── nebula_heart.rs # Pulsing nebula cores
│   │   ├── rain_glass.rs   # Raindrops on glass
│   │   └── willow_tears.rs # Weeping willow with tears
│   └── transparency.rs     # OSC 11 true transparency
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
