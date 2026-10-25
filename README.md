# xpaint

A simple, fast image editor for Linux, in the spirit of paint.net. Open a screenshot,
crop it, add arrows, boxes and text, and save it again.

![xpaint icon](assets/icon-256.png)

## Features

- **Tabs** for working on several images at once
- **Paste from clipboard** straight into a new image, which suits screenshots
- **Select and crop**
- **Drawing tools:** brush, arrow, rectangle, ellipse and text, with adjustable color, line width, font size and bold
- **Shapes and text stay editable** until you move on. Move them with the mouse or arrow keys, change their color or size, or delete them.
- **Undo / redo** for every edit
- **Zoom** with Ctrl + mouse wheel, the zoom slider, or fit to window
- **Opens** PNG, JPEG, BMP, GIF, TGA, TIFF, WebP, ICO and QOI
- **Saves** PNG, JPEG, WebP, BMP, GIF, TIFF, TGA and QOI. Transparent areas turn white when saving as JPEG.
- Works on Wayland and X11

## Installation

xpaint is installed from source. You need a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
git clone <repository-url> xpaint
cd xpaint
./install.sh
```

This builds xpaint and installs it for your user only (no root needed):

- the program at `~/.local/bin/xpaint`
- an application-menu entry and icon, so xpaint shows up in your launcher and in "Open with…" for image files

To remove it again:

```sh
./install.sh uninstall
```

## Usage

Start xpaint from your application menu, or from a terminal, optionally with images to open:

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
| Zoom in / out              | Ctrl + mouse wheel       |
| Close tab                  | Middle-click the tab     |

## License

xpaint is released under the [MIT License](LICENSE).

The bundled DejaVu fonts are under their own license; see `assets/fonts/DejaVu-LICENSE.txt`.
