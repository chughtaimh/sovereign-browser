# Sovereign Browser

A minimalist, privacy-focused web browser built with [Tauri v2](https://v2.tauri.app/) and vanilla HTML/CSS/JS.

## Features

- **Minimalist Design**: A clean, distraction-free interface that puts your content first.
- **Native Performance**: Built on Rust and Tauri for a lightweight footprint and blazing fast performance.
- **Integrated Ad Blocking**: Native, Rust-based ad blocking for faster, cleaner browsing.
- **Tab Management**: Support for multiple tabs with favicon syncing.
- **Deep Linking**: Handle links from external applications seamlessly.
- **Enhanced Privacy & Security**:
  - **Google Workspace Compatible**: Optimized user agent handling for full compatibility.
  - **Refined Security**: Hardened focus management and secure rendering.
- **Gesture Navigation**:
  - **Trackpad**: Two-finger swipe left/right to navigate back and forward.
  - **Mouse**: Support for side buttons (back/forward).
- **Keyboard Efficiency**:
  - `Cmd+L` or `Cmd+K`: Instantly focus the URL bar to search or navigate.
- **Suggestion System**: Built-in feedback mechanism to help improve the browser.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- Apple Xcode (for macOS development)

## Development

1. **Clone the repository**
   ```bash
   git clone https://github.com/chughtaimh/sovereign-browser.git
   cd sovereign-browser
   ```

2. **Run in development mode**
   ```bash
   cargo tauri dev
   ```

3. **Build for production**
   ```bash
   cargo tauri build
   ```

## Technology Stack

- **Backend**: Rust (Tauri), `adblock`
- **Frontend**: HTML5, CSS3, Vanilla JavaScript
- **Build Tool**: Cargo

## License

[MIT](LICENSE)
