# gh-downloader

A desktop application for browsing GitHub releases and downloading their assets.

Project website: https://luckyding-wj.github.io/gh-downloader/ (GitHub repository Settings → Pages must use GitHub Actions as the source.)

## Build

Requirements: Node.js 22+, Rust stable, and the platform prerequisites for Tauri 2.

```sh
npm ci
npm run dev:desktop
npm run build:desktop
```

The GitHub Actions workflow builds Windows and macOS bundles. Pushes to `main` upload workflow artifacts; pushing a version tag such as `v0.0.1` creates a GitHub Release with the Windows NSIS `.exe`, Windows `.msi`, and macOS `.dmg` installers attached. The tag version must match the app version in the manifests. Builds are unsigned; macOS may require allowing an unsigned app in system security settings. Windows builds use WebView2, normally installed on current Windows systems.

To publish the initial release after pushing the version changes:

```sh
git tag v0.0.1
git push origin v0.0.1
```

Linux is not built by this workflow. Building there requires Tauri's Linux system dependencies (including GTK/WebKitGTK); see the Tauri v2 prerequisites for your distribution.
