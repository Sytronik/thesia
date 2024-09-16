import {Menu, shell, BrowserWindow, MenuItemConstructorOptions, MenuItem} from "electron";
import {PLAY_BIG_JUMP_SEC, PLAY_JUMP_SEC} from "./constants";
import {showOpenDialog} from "./ipc";

interface DarwinMenuItemConstructorOptions extends MenuItemConstructorOptions {
  selector?: string;
  submenu?: DarwinMenuItemConstructorOptions[] | Menu;
}

type MenuItemClick = (
  menuItem: MenuItem,
  browserWindow: BrowserWindow | undefined,
  event: Electron.KeyboardEvent,
) => void;

const clickOpenMenu: MenuItemClick = async (_, browserWindow) =>
  browserWindow?.webContents.send("open-dialog-closed", await showOpenDialog());

const clickFreqZoomIn: MenuItemClick = (_, browserWindow, event) => {
  if (!event.triggeredByAccelerator) browserWindow?.webContents.send("freq-zoom-in");
};
const clickFreqZoomOut: MenuItemClick = (_, browserWindow, event) => {
  if (!event.triggeredByAccelerator) browserWindow?.webContents.send("freq-zoom-out");
};

const clickTimeZoomIn: MenuItemClick = (_, browserWindow) =>
  browserWindow?.webContents.send("time-zoom-in");
const clickTimeZoomOut: MenuItemClick = (_, browserWindow) =>
  browserWindow?.webContents.send("time-zoom-out");

const clickSelectAllTracks: MenuItemClick = (_, browserWindow) =>
  browserWindow?.webContents.send("select-all-tracks");

const clickRemoveTrackMenu: MenuItemClick = (_, browserWindow) =>
  browserWindow?.webContents.send("remove-selected-tracks");

const clickTogglePlayMenu: MenuItemClick = (_, browserWindow, event) => {
  if (!event.triggeredByAccelerator) browserWindow?.webContents.send("toggle-play");
};

const clickRewindToFront: MenuItemClick = (_, browserWindow, event) => {
  if (!event.triggeredByAccelerator) browserWindow?.webContents.send("rewind-to-front");
};

const clickJumpPlayerMenus = (
  browserWindow: BrowserWindow | undefined,
  event: Electron.KeyboardEvent,
  jumpSec: number,
) => {
  if (!event.triggeredByAccelerator) browserWindow?.webContents.send("jump-player", jumpSec);
};

export default class MenuBuilder {
  mainWindow: BrowserWindow;

  constructor(mainWindow: BrowserWindow) {
    this.mainWindow = mainWindow;
  }

  buildMenu(): Menu {
    if (process.env.NODE_ENV === "development" || process.env.DEBUG_PROD === "true")
      this.setupDevelopmentEnvironment();

    const template =
      process.platform === "darwin" ? this.buildDarwinTemplate() : this.buildDefaultTemplate();

    const menu = Menu.buildFromTemplate(template);
    Menu.setApplicationMenu(menu);

    return menu;
  }

  setupDevelopmentEnvironment(): void {
    this.mainWindow.webContents.on("context-menu", (_, props) => {
      const {x, y} = props;

      Menu.buildFromTemplate([
        {
          label: "Inspect element",
          click: () => this.mainWindow.webContents.inspectElement(x, y),
        },
      ]).popup({window: this.mainWindow});
    });
  }

  buildDarwinTemplate(): MenuItemConstructorOptions[] {
    const subMenuAbout: DarwinMenuItemConstructorOptions = {role: "appMenu"};
    const subMenuFile: DarwinMenuItemConstructorOptions = {
      label: "File",
      submenu: [
        {label: "Open Audio Tracks...", accelerator: "Command+O", click: clickOpenMenu},
        {type: "separator"},
        {role: "close"},
      ],
    };
    const subMenuEdit: DarwinMenuItemConstructorOptions = {
      id: "edit-menu",
      label: "Edit",
      submenu: [
        {label: "Undo", accelerator: "Command+Z", selector: "undo:", enabled: false},
        {label: "Redo", accelerator: "Shift+Command+Z", selector: "redo:", enabled: false},
        {type: "separator"},
        {label: "Cut", accelerator: "Command+X", selector: "cut:", enabled: false},
        {label: "Copy", accelerator: "Command+C", selector: "copy:", enabled: false},
        {label: "Paste", accelerator: "Command+V", selector: "paste:", enabled: false},
        {label: "Delete", selector: "delete:", enabled: false},
        {label: "Select All", accelerator: "Command+A", selector: "selectAll:", enabled: false},
      ],
    };
    const commonSubMenusForView: MenuItemConstructorOptions[] = [
      {
        id: "freq-zoom-in",
        label: "Frequency Zoom In",
        accelerator: "Command+Down",
        click: clickFreqZoomIn,
        enabled: false,
      },
      {
        id: "freq-zoom-out",
        label: "Frequency Zoom Out",
        accelerator: "Command+Up",
        click: clickFreqZoomOut,
        enabled: false,
      },
      {
        id: "time-zoom-in",
        label: "Time Zoom In",
        accelerator: "Command+Right",
        click: clickTimeZoomIn,
        enabled: false,
      },
      {
        id: "time-zoom-out",
        label: "Time Zoom Out",
        accelerator: "Command+Left",
        click: clickTimeZoomOut,
        enabled: false,
      },
      {type: "separator"},
    ];
    const devSubMenusForView: MenuItemConstructorOptions[] = [
      {
        label: "Reload",
        accelerator: "Command+R",
        click: () => this.mainWindow.webContents.reload(),
      },
      {
        label: "Toggle Full Screen",
        accelerator: "Ctrl+Command+F",
        click: () => this.mainWindow.setFullScreen(!this.mainWindow.isFullScreen()),
      },
      {
        label: "Toggle Developer Tools",
        accelerator: "Alt+Command+I",
        click: () => this.mainWindow.webContents.toggleDevTools(),
      },
    ];
    const prodSubMenusForView: MenuItemConstructorOptions[] = [
      {
        label: "Toggle Full Screen",
        accelerator: "Ctrl+Command+F",
        click: () => this.mainWindow.setFullScreen(!this.mainWindow.isFullScreen()),
      },
    ];
    const subMenuView: MenuItemConstructorOptions = {
      label: "View",
      submenu: commonSubMenusForView.concat(
        process.env.NODE_ENV === "development" || process.env.DEBUG_PROD === "true"
          ? devSubMenusForView
          : prodSubMenusForView,
      ),
    };
    const subMenuTracks: MenuItemConstructorOptions = {
      label: "Tracks",
      submenu: [
        {
          id: "select-all-tracks",
          label: "Select All Tracks",
          accelerator: "Command+A",
          click: clickSelectAllTracks,
        },
        {
          id: "remove-selected-tracks",
          label: "Remove Selected Tracks",
          accelerator: "Backspace",
          click: clickRemoveTrackMenu,
          enabled: false,
        },
        {
          label: "Remove Selected Tracks (Hidden)",
          accelerator: "Delete",
          visible: false,
          acceleratorWorksWhenHidden: true,
          click: clickRemoveTrackMenu,
        },
      ],
    };
    const subMenuPlay: MenuItemConstructorOptions = {
      id: "play-menu",
      label: "Play",
      submenu: [
        {
          id: "play",
          label: "Play",
          accelerator: "Space",
          click: clickTogglePlayMenu,
          enabled: false,
        },
        {
          id: "pause",
          label: "Pause",
          accelerator: "Space",
          click: clickTogglePlayMenu,
          visible: false,
        },
        {type: "separator"},
        {
          id: "rewind",
          label: `Rewind ${PLAY_JUMP_SEC}s`,
          accelerator: ",",
          click: (_, browserWindow, event) =>
            clickJumpPlayerMenus(browserWindow, event, -PLAY_JUMP_SEC),
          enabled: false,
        },
        {
          id: "fast-forward",
          label: `Fast forward ${PLAY_JUMP_SEC}s`,
          accelerator: ".",
          click: (_, browserWindow, event) =>
            clickJumpPlayerMenus(browserWindow, event, PLAY_JUMP_SEC),
          enabled: false,
        },
        {
          id: "rewind-big",
          label: `Rewind ${PLAY_BIG_JUMP_SEC}s`,
          accelerator: "Shift+,",
          click: (_, browserWindow, event) =>
            clickJumpPlayerMenus(browserWindow, event, -PLAY_BIG_JUMP_SEC),
          enabled: false,
        },
        {
          id: "fast-forward-big",
          label: `Fast forward ${PLAY_BIG_JUMP_SEC}s`,
          accelerator: "Shift+.",
          click: (_, browserWindow, event) =>
            clickJumpPlayerMenus(browserWindow, event, PLAY_BIG_JUMP_SEC),
          enabled: false,
        },
        {type: "separator"},
        {
          id: "rewind-to-front",
          label: "Rewind to the Front",
          accelerator: "Enter",
          click: clickRewindToFront,
          enabled: false,
        },
      ],
    };
    const subMenuWindow: MenuItemConstructorOptions = {role: "windowMenu"};
    const subMenuHelp: MenuItemConstructorOptions = {
      label: "Help",
      submenu: [
        {
          label: "Learn More",
          click: () => shell.openExternal("https://github.com/Sytronik/thesia"),
        },
        /* {
          label: "Documentation",
          click: () => shell.openExternal("https://github.com/Sytronik/thesia"),
        }, */
        /* {
          label: "Community Discussions",
          click: () => shell.openExternal("https://www.electronjs.org/community"),
        }, */
        {
          label: "Search Issues",
          click: () => shell.openExternal("https://github.com/Sytronik/thesia/issues"),
        },
      ],
    };

    return [
      subMenuAbout,
      subMenuFile,
      subMenuEdit,
      subMenuView,
      subMenuTracks,
      subMenuPlay,
      subMenuWindow,
      subMenuHelp,
    ];
  }

  buildDefaultTemplate() {
    const commonSubMenusForView: MenuItemConstructorOptions[] = [
      {
        id: "freq-zoom-in",
        label: "Frequency Zoom In",
        accelerator: "Ctrl+Down",
        click: clickFreqZoomIn,
        enabled: false,
        registerAccelerator: false,
      },
      {
        id: "freq-zoom-out",
        label: "Frequency Zoom Out",
        accelerator: "Ctrl+Up",
        click: clickFreqZoomOut,
        enabled: false,
        registerAccelerator: false,
      },
      {
        id: "time-zoom-in",
        label: "Time Zoom In",
        accelerator: "Ctrl+Right",
        click: clickTimeZoomIn,
        enabled: false,
      },
      {
        id: "time-zoom-out",
        label: "Time Zoom Out",
        accelerator: "Ctrl+Left",
        click: clickTimeZoomOut,
        enabled: false,
      },
      {type: "separator"},
    ];
    const templateDefault: MenuItemConstructorOptions[] = [
      {
        label: "&File",
        submenu: [
          {
            label: "&Open Audio Tracks...",
            accelerator: "Ctrl+O",
            click: async (_, browserWindow) =>
              browserWindow?.webContents.send("open-dialog-closed", await showOpenDialog()),
          },
          {type: "separator"},
          {role: "close"},
        ],
      },
      {
        label: "&View",
        submenu: commonSubMenusForView.concat(
          process.env.NODE_ENV === "development" || process.env.DEBUG_PROD === "true"
            ? [
                {
                  label: "&Reload",
                  accelerator: "Ctrl+R",
                  click: () => this.mainWindow.webContents.reload(),
                },
                {
                  label: "Toggle &Full Screen",
                  accelerator: "F11",
                  click: () => this.mainWindow.setFullScreen(!this.mainWindow.isFullScreen()),
                },
                {
                  label: "Toggle &Developer Tools",
                  accelerator: "Alt+Ctrl+I",
                  click: () => this.mainWindow.webContents.toggleDevTools(),
                },
              ]
            : [
                {
                  label: "Toggle &Full Screen",
                  accelerator: "F11",
                  click: () => this.mainWindow.setFullScreen(!this.mainWindow.isFullScreen()),
                },
              ],
        ),
      },
      {
        label: "&Tracks",
        submenu: [
          {
            id: "select-all-tracks",
            label: "Select &All Tracks",
            accelerator: "Ctrl+A",
            click: clickSelectAllTracks,
          },
          {
            id: "remove-selected-tracks",
            label: "&Remove Selected Tracks",
            accelerator: "Delete",
            click: clickRemoveTrackMenu,
            enabled: false,
          },
          {
            label: "Remove Selected Tracks (Hidden)",
            accelerator: "Backspace",
            visible: false,
            acceleratorWorksWhenHidden: true,
            click: clickRemoveTrackMenu,
          },
        ],
      },
      {
        id: "play-menu",
        label: "&Play",
        submenu: [
          {
            id: "play",
            label: "&Play",
            accelerator: "Space",
            click: clickTogglePlayMenu,
            enabled: false,
            registerAccelerator: false,
          },
          {
            id: "pause",
            label: "&Pause",
            accelerator: "Space",
            click: clickTogglePlayMenu,
            visible: false,
            registerAccelerator: false,
          },
          {type: "separator"},
          {
            id: "rewind",
            label: `Rewind ${PLAY_JUMP_SEC}s`,
            accelerator: ",",
            click: (_, browserWindow, event) =>
              clickJumpPlayerMenus(browserWindow, event, -PLAY_JUMP_SEC),
            enabled: false,
            registerAccelerator: false,
          },
          {
            id: "fast-forward",
            label: `Fast forward ${PLAY_JUMP_SEC}s`,
            accelerator: ".",
            click: (_, browserWindow, event) =>
              clickJumpPlayerMenus(browserWindow, event, PLAY_JUMP_SEC),
            enabled: false,
            registerAccelerator: false,
          },
          {
            id: "rewind-big",
            label: `Rewind ${PLAY_BIG_JUMP_SEC}s`,
            accelerator: "Shift+,",
            click: (_, browserWindow, event) =>
              clickJumpPlayerMenus(browserWindow, event, -PLAY_BIG_JUMP_SEC),
            enabled: false,
            registerAccelerator: false,
          },
          {
            id: "fast-forward-big",
            label: `Fast forward ${PLAY_BIG_JUMP_SEC}s`,
            accelerator: "Shift+.",
            click: (_, browserWindow, event) =>
              clickJumpPlayerMenus(browserWindow, event, PLAY_BIG_JUMP_SEC),
            enabled: false,
            registerAccelerator: false,
          },
          {type: "separator"},
          {
            id: "rewind-to-front",
            label: "&Rewind to the Front",
            accelerator: "Enter",
            click: clickRewindToFront,
            enabled: false,
            registerAccelerator: false,
          },
        ],
      },
      {
        label: "Help",
        submenu: [
          {
            label: "&Learn More",
            click: () => shell.openExternal("https://github.com/Sytronik/thesia"),
          },
          /* {
            label: "Documentation",
            click: () => shell.openExternal("https://github.com/Sytronik/thesia"),
          }, */
          /* {
            label: "Community Discussions",
            click: () => shell.openExternal("https://www.electronjs.org/community"),
          }, */
          {
            label: "&Search Issues",
            click: () => shell.openExternal("https://github.com/Sytronik/thesia/issues"),
          },
        ],
      },
    ];

    return templateDefault;
  }
}
