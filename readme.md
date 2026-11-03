# UniTeach

UniTeach is a Windows desktop overlay that uses AI to read questions from your screen and display answers, explanations, and solution steps. Built with Rust and egui, it connects to OpenRouter for screenshot analysis and question solving.

## Features

- Resizable, always-on-top overlay with scrollable settings and answers.
- Monitor selection and capture intervals of 5, 10, 15, 20, or 30 seconds.
- Separate vision and reasoning models, with a searchable live OpenRouter catalogue ordered by popularity, most popular first.
- All answer choices displayed, with recommended choices highlighted.
- Keyboard shortcuts for visibility and mouse interaction.
- Windows capture exclusion for the overlay during a session, where supported.

## Requirements

- Windows with a connected display.
- A current stable [Rust toolchain](https://rustup.rs/) and the Microsoft C++ build tools required by Rust's Windows MSVC toolchain.
- Internet access and an [OpenRouter API key](https://openrouter.ai/settings/keys) with access to your selected models.

## Build and run

From the project folder:

```powershell
cargo run --locked --release
```

To build an executable for later use:

```powershell
cargo build --locked --release
.\target\release\uniteach.exe
```

## How to use

1. Choose the monitor you want to capture.
2. Enter your OpenRouter API key and choose a capture interval.
3. Choose a **vision model** that accepts images and a **reasoning model** that supports text chat. Unsupported models appear disabled. Search by model or provider name, or use **Refresh models** to reload the catalogue.
4. Click **Start session**. UniTeach periodically captures the selected screen, extracts a visible question, and displays the suggested answer and explanation. Expand **Solution steps** for more detail.
5. Click **Stop** to end the session and return to settings. Drag the title bar to move the window or the bottom-right grip to resize it. Scroll to reach settings or answers that exceed the window height.

| Shortcut | Action |
| --- | --- |
| `Ctrl+Shift+H` | Show or hide the overlay |
| `Ctrl+Shift+G` | Toggle click-through mode (mouse interaction) |

If click-through is enabled, press `Ctrl+Shift+G` again to interact with the overlay controls.

Screenshots are sent to the selected vision model through OpenRouter, and extracted questions are sent to the reasoning model. Requests may incur charges according to the models you choose. The API key is kept in memory for the current app run; it is not saved to a settings file.

## Development

```powershell
cargo fmt --check
cargo check --locked
cargo test --locked
```

The live catalogue integration test is skipped by default. To run it with internet access:

```powershell
cargo test --locked live_catalogue_contains_chat_and_vision_models -- --ignored --nocapture
```

The source is organized into `app` (application state), `ui` (setup and overlay screens), `models` (OpenRouter catalogue), `ai` (OpenRouter requests), `capture` (screenshots), `session` (session state), and `stealth` (Windows window controls).
