import {
  app,
  Menu,
  shell,
  BrowserWindow,
  MenuItemConstructorOptions,
  dialog,
  MenuItem,
} from "electron";
import path from "path";
import {SUPPORTED_TYPES} from "./constants";

export const showOpenDialog = () =>
  dialog.showOpenDialog({
    title: "Select the audio files to be open",
    defaultPath: path.join(__dirname, "../../samples/"),
    filters: [
      {
        name: "Audio Files",
        extensions: SUPPORTED_TYPES,
      },
    ],
    properties: ["openFile", "multiSelections"],
  });

const clickOpenMenu = async (menuItem: MenuItem, browserWindow: BrowserWindow | undefined) =>
  browserWindow?.webContents.send("open-dialog-closed", await showOpenDialog());

const clickRemoveTrackMenu = (menuItem: MenuItem, browserWindow: BrowserWindow | undefined) =>
  browserWindow?.webContents.send("delete-track");

interface DarwinMenuItemConstructorOptions extends MenuItemConstructorOptions {
  selector?: string;
  submenu?: DarwinMenuItemConstructorOptions[] | Menu;
}

export default class MenuBuilder {
  mainWindow: BrowserWindow;

  constructor(mainWindow: BrowserWindow) {
    this.mainWindow = mainWindow;
  }

  buildMenu(): Menu {
    if (process.env.NODE_ENV === "development" || process.env.DEBUG_PROD === "true") {
      this.setupDevelopmentEnvironment();
    }

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
          click: () => {
            this.mainWindow.webContents.inspectElement(x, y);
          },
        },
      ]).popup({window: this.mainWindow});
    });
  }

  buildDarwinTemplate(): MenuItemConstructorOptions[] {
    const subMenuAbout: DarwinMenuItemConstructorOptions = {
      label: "Electron",
      submenu: [
        {
          label: "About Thesia",
          selector: "orderFrontStandardAboutPanel:",
        },
        {type: "separator"},
        {label: "Services", submenu: []},
        {type: "separator"},
        {
          label: "Hide Thesia",
          accelerator: "Command+H",
          selector: "hide:",
        },
        {
          label: "Hide Others",
          accelerator: "Command+Shift+H",
          selector: "hideOtherApplications:",
        },
        {label: "Show All", selector: "unhideAllApplications:"},
        {type: "separator"},
        {
          label: "Quit",
          accelerator: "Command+Q",
          click: () => {
            app.quit();
          },
        },
      ],
    };
    const subMenuFile: DarwinMenuItemConstructorOptions = {
      label: "File",
      submenu: [
        {
          label: "Open Audio Tracks...",
          accelerator: "Command+O",
          selector: "open:",
          click: clickOpenMenu,
        },
        {
          label: "Remove Selected Tracks",
          accelerator: "Backspace",
          click: clickRemoveTrackMenu,
        },
        {
          label: "Remove Selected Tracks (Hidden)",
          accelerator: "Delete",
          visible: false,
          acceleratorWorksWhenHidden: true,
          click: clickRemoveTrackMenu,
        },
        {type: "separator"},
        {
          role: "close",
        },
      ],
    };
    const subMenuViewDev: MenuItemConstructorOptions = {
      label: "View",
      submenu: [
        {
          label: "Reload",
          accelerator: "Command+R",
          click: () => {
            this.mainWindow.webContents.reload();
          },
        },
        {
          label: "Toggle Full Screen",
          accelerator: "Ctrl+Command+F",
          click: () => {
            this.mainWindow.setFullScreen(!this.mainWindow.isFullScreen());
          },
        },
        {
          label: "Toggle Developer Tools",
          accelerator: "Alt+Command+I",
          click: () => {
            this.mainWindow.webContents.toggleDevTools();
          },
        },
      ],
    };
    const subMenuViewProd: MenuItemConstructorOptions = {
      label: "View",
      submenu: [
        {
          label: "Toggle Full Screen",
          accelerator: "Ctrl+Command+F",
          click: () => {
            this.mainWindow.setFullScreen(!this.mainWindow.isFullScreen());
          },
        },
      ],
    };
    const subMenuWindow: DarwinMenuItemConstructorOptions = {
      label: "Window",
      submenu: [
        {
          label: "Minimize",
          accelerator: "Command+M",
          selector: "performMiniaturize:",
        },
        {label: "Close", accelerator: "Command+W", selector: "performClose:"},
        {type: "separator"},
        {label: "Bring All to Front", selector: "arrangeInFront:"},
      ],
    };
    const subMenuHelp: MenuItemConstructorOptions = {
      label: "Help",
      submenu: [
        {
          label: "Learn More",
          click() {
            shell.openExternal("https://github.com/Sytronik/thesia");
          },
        },
        /* {
          label: "Documentation",
          click() {
            shell.openExternal("https://github.com/Sytronik/thesia");
          },
        }, */
        /* {
          label: "Community Discussions",
          click() {
            shell.openExternal("https://www.electronjs.org/community");
          },
        }, */
        {
          label: "Search Issues",
          click() {
            shell.openExternal("https://github.com/Sytronik/thesia/issues");
          },
        },
      ],
    };

    const subMenuView =
      process.env.NODE_ENV === "development" || process.env.DEBUG_PROD === "true"
        ? subMenuViewDev
        : subMenuViewProd;

    return [subMenuAbout, subMenuFile, subMenuView, subMenuWindow, subMenuHelp];
  }

  buildDefaultTemplate() {
    const templateDefault: MenuItemConstructorOptions[] = [
      {
        label: "&File",
        submenu: [
          {
            label: "&Open Audio Tracks...",
            accelerator: "Ctrl+O",
            click: async (menuItem, browserWindow) =>
              browserWindow?.webContents.send("open-dialog-closed", await showOpenDialog()),
          },
          {
            label: "&Remove Selected Tracks",
            accelerator: "Delete",
            click: clickRemoveTrackMenu,
          },
          {
            label: "Remove Selected Tracks (Hidden)",
            accelerator: "Backspace",
            visible: false,
            acceleratorWorksWhenHidden: true,
            click: clickRemoveTrackMenu,
          },
          {
            label: "&Close",
            accelerator: "Ctrl+W",
            click: () => {
              this.mainWindow.close();
            },
          },
        ],
      },
      {
        label: "&View",
        submenu:
          process.env.NODE_ENV === "development" || process.env.DEBUG_PROD === "true"
            ? [
                {
                  label: "&Reload",
                  accelerator: "Ctrl+R",
                  click: () => {
                    this.mainWindow.webContents.reload();
                  },
                },
                {
                  label: "Toggle &Full Screen",
                  accelerator: "F11",
                  click: () => {
                    this.mainWindow.setFullScreen(!this.mainWindow.isFullScreen());
                  },
                },
                {
                  label: "Toggle &Developer Tools",
                  accelerator: "Alt+Ctrl+I",
                  click: () => {
                    this.mainWindow.webContents.toggleDevTools();
                  },
                },
              ]
            : [
                {
                  label: "Toggle &Full Screen",
                  accelerator: "F11",
                  click: () => {
                    this.mainWindow.setFullScreen(!this.mainWindow.isFullScreen());
                  },
                },
              ],
      },
      {
        label: "Help",
        submenu: [
          {
            label: "Learn More",
            click() {
              shell.openExternal("https://github.com/Sytronik/thesia");
            },
          },
          /* {
            label: "Documentation",
            click() {
              shell.openExternal("https://github.com/Sytronik/thesia");
            },
          }, */
          /* {
            label: "Community Discussions",
            click() {
              shell.openExternal("https://www.electronjs.org/community");
            },
          }, */
          {
            label: "Search Issues",
            click() {
              shell.openExternal("https://github.com/Sytronik/thesia/issues");
            },
          },
        ],
      },
    ];

    return templateDefault;
  }
}
