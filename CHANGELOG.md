# Changelog

All notable changes to Soplos Layout Switcher are documented in this file.

## [3.0.0-2-beta] - 2026-10-31

### Changed

- The Settings page is no longer a tab beside the layouts. A gear at the bottom right of the banner opens it as a sidebar at the left of the layouts: the window grows to make room for it, in a few short steps, and gives it back when the sidebar is closed with its button, the Escape key or the gear. The window is only resized, never moved. A maximized window is not resized: the sidebar takes room from the layouts. Its switches and tools are in one column.
- The window is shorter, 664 pixels, now that the tabs are gone, and the buttons under the layouts are smaller.
- The arrow of the language selector no longer covers the name of the language.

### Fixed

- Reload Desktop decides between closing the session and restarting the shell from the type of the session, as the program already read it. It looked for a running Xwayland before, which GNOME starts only when an application asks for it, so a Wayland session without one was taken for an X11 one and the button failed. When the shell does not accept the restart through D-Bus, which GNOME refuses since version 41, the shell of the user is asked to restart with a signal.

## [3.0.0-1-beta] - 2026-10-29

### Changed

- The window no longer has the frame of the system: it is a card with rounded corners and its own minimize, maximize and close buttons at the top right. It can be moved from the top strip, the banner and the side bar, and double click on the strip maximizes it. The rounded corners need a compositing manager running, as the one of Xfce, Plasma or GNOME.

## [3.0.0-beta] - 2026-10-29

First beta, the version being tested on real hardware.

### Added

- A picture for each layout you save, drawn from what it keeps in the style of the pictures of the four layouts of the program, where it had a generic icon: the top bar or Dash to Panel on its side, Dash to Dock on its side or along all of it, the windows as tiles with Forge, Pop Shell, Material Shell or Tiling Shell, the ArcMenu button, Open Bar islands and the desktop icons. An extension the program does not know is not drawn.

### Changed

- The layouts are in columns of 180 to 220 wide, as many as the window holds, centred: two rows of two in the window as it opens, and more columns when it is made bigger or a tiling shell gives it the whole screen, where the two columns stayed lost in the middle.

## [3.0.0-alpha] - 2026-10-28

Full rewrite of Soplos Layout Switcher, from Python 3 and GTK3 to Tauri 2 with a Rust backend and a Svelte frontend. See the Architecture section of the README. Not yet tested on real hardware.

### Added

- Application icon and hero image of the new Soplos 3.0 artwork, with the circuit style of the other Soplos 3.0 programs.
- Screenshots of the Layout page, the Settings page and the dialog that saves a layout, taken with the interface in English.
- Rust backend (`src-tauri/`) replacing the Python 3 and GTK3 codebase. The four layouts of the 2.x program, Soplos, Dock, Tiling and GNOME, are kept as data with the extensions each one needs, the extensions of the other layouts it turns off, and the settings it applies, all as they were in the 2.x program: the lists were checked on a real Soplos Boro install. Settings are applied with `gsettings`, and with `dconf` for the extensions installed only in the user's folder. Nothing goes through a shell.
- Svelte 4 and Tailwind CSS frontend (`src/`) with the two pages of the 2.x program: Layout, with a picture of each layout and the buttons Apply and Reload Desktop, and Settings, with the switches of the system tray and the desktop icons and the buttons that open GNOME Tweaks, the extensions manager, GNOME Settings (accent color), GDM Settings and Add Water. The frame is the one of the other Soplos 3.0 programs without a sidebar: a hero, a thin footer with the theme button, the session and the version, and a progress bar along the bottom edge while a job runs.
- Installation of the extensions a layout needs before it is applied, as the 2.x program did: from the Debian package, from the extensions that ship with the program (Forge and Open Bar), or from extensions.gnome.org; what cannot be installed is left to the user in the extensions manager. The packages are installed as administrator in one go, so the password is asked once and not once for each package, through the same executable started with `pkexec` and a closed list of the five packages the program knows, checked again as root.
- Layouts of your own: the Layout page has a button to save the desktop as it is now with a name. It keeps the extensions that are on, the settings of all the extensions (`dconf dump /org/gnome/shell/extensions/`) and the window buttons, in `~/.config/soplos-layout-switcher/layouts/`, one JSON file for each. A saved layout is a card after the four of the program, with a button to delete it, and is applied as the others are: the missing extensions are installed, the settings are put back with `dconf load`, and the extensions that are not in the layout are turned off and the ones that are, on. A name that is already there asks before it is replaced. The files are checked again when they are read, since they can be edited by hand, and `soplos-layout-switcher-cli apply-<id>` applies them too.
- Refine, the alternative to GNOME Tweaks, beside it in Settings. It has no Debian package: it is the Flatpak `page.tesk.Refine`, installed for the user from Flathub the first time, as Add Water is.
- A failure or a note is shown in a dialog with what happened, translated: an extension that cannot be turned on yet because the shell has not seen it, one to install by hand, a setting that did not change. The window has no log.
- Command line, `soplos-layout-switcher-cli` with `apply-soplos`, `apply-dock`, `apply-gnome`, `apply-tiling` and `reload-gnome-shell`, as in the 2.x program, for scripts and installers.
- The layout applied last is kept in `~/.config/soplos-layout-switcher.conf`, the file of the 2.x program, so a layout chosen with that version is still the one shown.
- The window follows the light or dark theme of the desktop, with a button in the footer to choose.
- Interface in eight languages: English, Spanish, German, French, Italian, Portuguese, Romanian and Russian. It starts in the system language and falls back to English.
- Unix manual page (`soplos-layout-switcher.1`), AppStream metainfo and desktop entry in the eight languages, Polkit policy, Debian copyright (declaring Lucide, ISC, and the two bundled extensions) and this `CHANGELOG.md` installed as `/usr/share/doc/soplos-layout-switcher/changelog.gz`.

### Changed

- The program asks for the password once when several packages are missing, and says when an extension or a Flatpak app it was going to open is not installed, instead of failing without a word.
- The four layouts are in two rows of two, a little smaller than in the 2.x program, and the window opens 700 x 700 and not 700 x 600, so the Layout page does not need the scroll bar.
- The program reports when an extension could not be turned on, where the 2.x program always said it was done.
