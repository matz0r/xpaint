# xpaint

A simple, fast image editor for Linux and Windows in a single binary. Open a screenshot,
crop it, add arrows, boxes and text, and save it again.

<p align="center">
    <img src="assets/screenshot.png" alt="xpaint with an annotated screenshot open" width="320">
</p>

## Download
Prebuilt binaries for Linux and Windows are attached to each [release](https://github.com/matz0r/xpaint/releases).

### Optional install
<p align="center">
    <img src="assets/install.png" alt="how to install xpaint" width="175">
</p>

## Manual Installation

xpaint is a single program file with nothing else to install.

1. Download the latest `xpaint-…-x86_64-linux.tar.gz` (Linux) or
   `xpaint-…-x86_64-windows.zip` (Windows) from the
   [releases page](https://github.com/matz0r/xpaint/releases) and unpack it.
2. Run `xpaint` (`xpaint.exe` on Windows) from the unpacked folder: double-click it,
   or run `./xpaint` in a terminal.

That's it. It runs on 64-bit Linux distributions from about 2024 on and on 64-bit
Windows 10 and 11. The Windows build isn't signed, so SmartScreen may warn on
the first start: choose **More info → Run anyway**.

### Adding xpaint to your application menu

To start xpaint like any other app, open **Settings** (the sliders icon at the top
right) and choose **Install for this user**. This:

- copies xpaint to `~/.local/bin/xpaint`, so `xpaint` also works in a terminal
- adds it to your application menu, with its icon
- offers it in "Open with…" for image files

On Windows it copies xpaint to `%LOCALAPPDATA%\Programs\xpaint`, adds it to the
Start menu and offers it in "Open with" for images.

It's only for your user and needs no root password or admin rights. Afterwards you
can delete the downloaded folder.

To remove xpaint again, choose **Settings → Uninstall**.

### Updates

The installed copy has **Check for updates once a day** in the **Settings** menu,
off by default. Turn it on and it checks GitHub for a newer release right away;
after that it checks when xpaint starts, at most once every 24 hours. If there is
a newer version, **Update to x.y.z** appears in the toolbar: it downloads that
release and installs it, then offers to restart (open images are reopened from
their files). Nothing is downloaded or installed without that click, and with the
setting off no update check is made. `xpaint --update` updates from the command
line in any case.

You can also update by hand: start a newly downloaded xpaint and choose
**Settings → Update installation**. Versions up to 0.3.0 don't have the update
setting yet: install the new download once that way.

## Features

- **Tabs** for working on several images at once
- **Paste from clipboard** straight into a new image, which suits screenshots
- **Select and crop**
- **Drawing tools:** brush, arrow, rectangle, ellipse and text, with adjustable color, line width, font size and bold
- **Shapes and text stay editable** until you move on. Move them with the mouse or arrow keys, change their color or size, or delete them.
- **Undo / redo** for every edit
- **Zoom** with Ctrl + mouse wheel, the zoom slider, fit to window, or back to 100%
- **Opens** PNG, JPEG, BMP, GIF, TGA, TIFF, WebP, ICO and QOI
- **Saves** PNG, JPEG, WebP, BMP, GIF, TIFF, TGA and QOI. Transparent areas turn white when saving as JPEG.
- **Adjustable icon size** (Normal, Large, Larger) in the Settings menu, remembered between runs
- **Installs itself** into your application menu from the Settings menu
- Works on Wayland and X11, and on Windows

## Usage

Start xpaint from your application menu (once installed), or from a terminal, optionally with images to open:

```sh
xpaint screenshot.png photo.jpg
```

Images given on the command line each open in their own tab.

### Tools

| Tool             | Key | Notes                                                       |
|------------------|-----|-------------------------------------------------------------|
| Rectangle Select | S   | Drag to select an area                                      |
| Brush            | B   | Freehand drawing; a click paints a dot                      |
| Arrow            | A   | Drag from the start to the arrow tip                        |
| Rectangle        | R   | Hold Shift for a square                                     |
| Ellipse          | E   | Hold Shift for a circle                                     |
| Text             | T   | Click to place text, then type; Enter starts a new line     |

A shape or text you just made floats above the image until you commit it:

- **Drag** it, or use the **arrow keys** (1 pixel; hold Shift for 10 pixels), to move it
- Change the color, width or font size in the toolbar to restyle it
- **Delete** removes it
- **Esc** commits it to the image. Switching tools, saving, cropping and undo commit it too.

While typing text, **Esc** stops typing so you can move or restyle the text.

### Keyboard shortcuts

Most shortcuts are the same as in paint.net.

| Action                     | Shortcut                 |
|----------------------------|--------------------------|
| Open image                 | Ctrl+O                   |
| Save                       | Ctrl+S                   |
| Save as                    | Ctrl+Shift+S             |
| New image from clipboard   | Ctrl+Alt+V               |
| Undo                       | Ctrl+Z                   |
| Redo                       | Ctrl+Y or Ctrl+Shift+Z   |
| Crop to selection          | Ctrl+Shift+X             |
| Deselect                   | Ctrl+D or Esc            |
| Zoom to fit window         | Ctrl+B                   |
| Zoom to 100%               | Ctrl+0                   |
| Zoom in / out              | Ctrl + mouse wheel       |
| Close tab                  | Middle-click the tab     |

## Development

Building from source, running the tests and making releases is described in
[DEVELOPMENT.md](DEVELOPMENT.md).

## License

xpaint is released under the [MIT License](LICENSE).

The bundled DejaVu fonts are under their own license; see `assets/fonts/DejaVu-LICENSE.txt`.
