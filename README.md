# vim-sandbox

A minimalist Vim-like editor with artistic animated backgrounds. Written in Rust with ratatui.

## Features

- **Vim-like editing** — Normal, Insert, Visual, Command modes with motions, operators, registers, undo/redo
- **1 artistic theme** — `spider-lily` procedural animated background (9 more planned)
- **True transparency** — Works with compositors (picom, hyprland, etc.) via OSC 11 sequences
- **File operations** — `:e`, `:w`, `:q`, `:wq`, search `/`, tabs completion for themes
- **Zero dependencies on emoji** — Pure ASCII/Unicode Braille characters

## Themes

| Name | Description |
|------|-------------|
| `spider-lily` | Red spider lilies blooming/wilting, petals drifting (Tokyo Ghoul aesthetic) |

*9 other themes planned (`sakura-night`, `abyssal-bloom`, `firefly-dusk`, `aurora-void`, `koi-dreams`, `bamboo-whisper`, `nebula-heart`, `rain-glass`, `willow-tears`) — removed from the current branch, available in `git show HEAD~1:src/ui/themes.rs`.*

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
:themes                      " List available themes (1 theme: spider-lily)
:theme spider-lily           " Set theme (stored as status, no visual change yet)
:colorscheme aurora-void     " Alias
:themenext / :themeprev      " Next / Previous theme (no-op: single theme)
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
│   ├── themes.rs           # 1 procedural ASCII theme (spider-lily)
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