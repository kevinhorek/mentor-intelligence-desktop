# Mentor Intelligence for desktop

The Mentor Intelligence desktop app for Windows and Linux. It also builds on macOS.

It is a small window around [www.mentorintell.com](https://www.mentorintell.com), built with [Tauri 2](https://v2.tauri.app). It works like the iOS and Android apps: the site does the work, the app gives it its own window.

## What it does

- Opens on the sign-in page. If you are already signed in, the site takes you to your dashboard.
- Keeps Mentor Intelligence pages and the Google, Apple and Microsoft sign-in pages inside the window. Every other link opens in your normal browser.
- Saves downloads (PDF and CSV exports) to your Downloads folder and shows the file when it is done.
- Remembers the window size and place.
- Runs once. Opening it again brings the open window to the front.
- Shows a short offline message with a Retry button when the site cannot be reached.
- Adds `MentorIntelligenceDesktop/1.0.0` to the browser's user agent, so the site can tell the app apart from a browser.

## Safety

- The site gets no access to the computer. It cannot call any app command. The only app pages with any access are the ones bundled in `dist/`, and even they get none (`src-tauri/capabilities/local-shell.json`).
- There are no secrets in this repository. Nothing here needs a key.

## Download

Get the latest installer from the GitHub Releases page of this repository.

| System | File |
| --- | --- |
| Windows | `Mentor Intelligence_1.0.0_x64-setup.exe` (or the `.msi`) |
| Ubuntu, Debian | `Mentor Intelligence_1.0.0_amd64.deb` |
| Fedora, RHEL, openSUSE | `Mentor Intelligence-1.0.0-1.x86_64.rpm` |
| Any Linux | `Mentor Intelligence_1.0.0_amd64.AppImage` |
| macOS | `Mentor Intelligence_1.0.0_aarch64.dmg` |

Windows may warn that the app is from an unknown publisher until the installers are code signed (see below).

On Linux, make the AppImage runnable first: `chmod +x "Mentor Intelligence_1.0.0_amd64.AppImage"`, then open it.

## Build it yourself

You need Node.js 20 or newer and Rust (stable). On Linux you also need the WebKitGTK packages:

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

Then:

```sh
npm ci
npm run tauri dev      # run it from source
npm run tauri build    # make installers for the system you are on
cargo test --manifest-path src-tauri/Cargo.toml   # link rules tests
```

Installers land in `src-tauri/target/release/bundle/`. Each system builds its own installers: Windows makes `.msi` and `.exe`, Linux makes `.deb`, `.rpm` and `.AppImage`, macOS makes `.app` and `.dmg`.

To change the icon, replace `src-tauri/app-icon.png` (1024 by 1024) and run `npm run icons`.

## Releases

Push a tag that starts with `v`, for example `v1.0.0`. The workflow in `.github/workflows/release.yml` builds on Windows, Ubuntu 22.04 and macOS and attaches the installers to a draft GitHub Release. Check the draft, then publish it.

Before tagging, set the same version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.

## Microsoft Store

Tauri 2 does not make MSIX packages. The Microsoft Store accepts regular installers through its "MSI or EXE app" route, so we submit the `.msi`.

Steps:

1. Create a Microsoft Partner Center developer account (Kevin). Use the company as the publisher.
2. In Partner Center, choose Apps and games, New product, "MSI or EXE app". Reserve the name "Mentor Intelligence".
3. Code sign the `.msi` with a certificate from a trusted certificate authority. The Store requires this for MSI and EXE apps. Signing goes in `bundle.windows.certificateThumbprint` (or a signing command) in `src-tauri/tauri.conf.json`, and the certificate stays out of this repository.
4. Give the Store a direct HTTPS link to that exact `.msi` version. A GitHub Release asset link works. The file at that link must never change; each new version gets a new link.
5. Silent install: the MSI supports `/quiet` on its own, so put `/quiet` in the installer parameters.
6. Privacy policy URL: https://www.mentorintell.com/privacy
7. Fill in the listing: description, screenshots, category (Business), and age rating.
8. Submit for certification.

Only Kevin can do steps 1, 2, 3 and 8: the account, the publisher identity, buying the signing certificate and submitting.

If the Store later needs MSIX, the signed app can be packaged with Microsoft's MSIX Packaging Tool from the MSI.

## Linux notes

- `.deb` for Ubuntu and Debian, `.rpm` for Fedora and openSUSE, `.AppImage` for everything else.
- The app needs WebKitGTK 4.1. The `.deb` and `.rpm` pull it in. The AppImage carries what it needs.
- Flathub can come later. It needs a Flatpak manifest and a Flathub submission.

## macOS notes

The macOS build is for local testing. It is not signed or notarized, so macOS will ask before opening it the first time (right-click, Open).

## License

No license has been chosen yet. Until one is added, all rights are reserved.
