/* eslint global-require: off, no-console: off, promise/always-return: off */

/**
 * This module executes inside of electron's main process. You can start
 * electron renderer process from here and communicate with the other processes
 * through IPC.
 *
 * When running `npm run build` or `npm run build:main`, this file is compiled to
 * `./src/main.js` using webpack. This gives us some performance wins.
 */
import path from "path";
import {
  app,
  BrowserWindow,
  shell,
  ipcMain,
  dialog,
  Menu,
  MenuItemConstructorOptions,
} from "electron";
import {autoUpdater} from "electron-updater";
import log from "electron-log";
import os from "os";
import MenuBuilder, {showOpenDialog} from "./menu";
import {resolveHtmlPath} from "./util";
import {SUPPORTED_TYPES} from "./constants";

class AppUpdater {
  constructor() {
    log.transports.file.level = "info";
    autoUpdater.logger = log;
    autoUpdater.checkForUpdatesAndNotify();
  }
}

let mainWindow: BrowserWindow | null = null;

ipcMain.on("show-open-dialog", async (event) => {
  event.reply("open-dialog-closed", await showOpenDialog());
});

ipcMain.on("show-file-open-err-msg", async (event, unsupportedPaths, invalidPaths) => {
  const msgUnsupported = unsupportedPaths.length
    ? `-- Not Supported Type --
      ${unsupportedPaths.join("\n")}
      `
    : "";
  const msgInvalid = invalidPaths.length
    ? `-- Not Valid Format --
      ${invalidPaths.join("\n")}
      `
    : "";
  await dialog.showMessageBox({
    type: "error",
    buttons: ["OK"],
    defaultId: 0,
    title: "File Open Error",
    message: "The following files could not be opened",
    detail: `${msgUnsupported}\
        ${msgInvalid}\

        Please ensure that the file properties are correct and that it is a supported file type.
        Only files with the following extensions are allowed: ${SUPPORTED_TYPES.join(", ")}`,
    cancelId: 0,
    noLink: false,
    normalizeAccessKeys: false,
  });
});

ipcMain.on("show-track-context-menu", (event) => {
  const template: MenuItemConstructorOptions[] = [
    {
      label: `Remove Selected Tracks\t(${os.platform() === "darwin" ? "⌫|⌦" : "Delete|Backspace"})`,
      click: () => {
        event.sender.send("remove-selected-tracks");
      },
    },
    {
      label: `Select All Tracks\t\t\t(${os.platform() === "darwin" ? "⌘+A" : "Ctrl+A"})`,
      click: () => event.sender.send("select-all-tracks"),
    },
  ];
  const menu = Menu.buildFromTemplate(template);
  menu.popup({window: BrowserWindow.fromWebContents(event.sender) ?? undefined});
});

ipcMain.on("show-axis-context-menu", (event, axisKind) => {
  const template: MenuItemConstructorOptions[] = [];
  if (axisKind === "ampAxis") {
    template.push({
      label: "Edit Range\t\t(Double Click)",
      click: () => {
        event.sender.send("edit-axis-range", axisKind);
      },
    });
  } else if (axisKind === "freqAxis") {
    template.push(
      {
        label: "Edit Upper Limit\t(Double Click)",
        click: () => {
          event.sender.send("edit-axis-range", axisKind, "max");
        },
      },
      {
        label: "Edit Lower Limit\t(Double Click)",
        click: () => {
          event.sender.send("edit-axis-range", axisKind, "min");
        },
      },
    );
  }
  template.push({
    label: `Reset Range\t\t(${os.platform() === "darwin" ? "⌥+Click" : "Alt+Click"})`,
    click: () => {
      event.sender.send("reset-axis-range", axisKind);
    },
  });
  const menu = Menu.buildFromTemplate(template);
  menu.popup({window: BrowserWindow.fromWebContents(event.sender) ?? undefined});
});

ipcMain
  .on("enable-remove-track-menu", () => {
    const removeTrackMenu = Menu.getApplicationMenu()?.getMenuItemById("remove-selected-tracks");
    if (removeTrackMenu) removeTrackMenu.enabled = true;
  })
  .on("disable-remove-track-menu", () => {
    const removeTrackMenu = Menu.getApplicationMenu()?.getMenuItemById("remove-selected-tracks");
    if (removeTrackMenu) removeTrackMenu.enabled = false;
  });

ipcMain
  .on("show-play-menu", () => {
    const appMenu = Menu.getApplicationMenu();
    if (appMenu === null) return;
    const togglePlayMenu = appMenu.getMenuItemById("play");
    if (togglePlayMenu) togglePlayMenu.visible = true;
    const togglePauseMenu = appMenu.getMenuItemById("pause");
    if (togglePauseMenu) togglePauseMenu.visible = false;
  })
  .on("show-pause-menu", () => {
    const appMenu = Menu.getApplicationMenu();
    if (appMenu === null) return;
    const togglePlayMenu = appMenu.getMenuItemById("play");
    if (togglePlayMenu) togglePlayMenu.visible = false;
    const togglePauseMenu = appMenu.getMenuItemById("pause");
    if (togglePauseMenu) togglePauseMenu.visible = true;
  })
  .on("enable-play-menu", () => {
    const playMenu = Menu.getApplicationMenu()?.getMenuItemById("play-menu");
    if (playMenu) {
      playMenu.submenu?.items.forEach((item) => {
        item.enabled = true;
      });
    }
  })
  .on("disable-play-menu", () => {
    const playMenu = Menu.getApplicationMenu()?.getMenuItemById("play-menu");
    if (playMenu) {
      playMenu.submenu?.items.forEach((item) => {
        item.enabled = false;
      });
    }
  });

if (process.env.NODE_ENV === "production") {
  const sourceMapSupport = require("source-map-support");
  sourceMapSupport.install();
}

const isDebug = process.env.NODE_ENV === "development" || process.env.DEBUG_PROD === "true";

if (isDebug) {
  require("electron-debug")();
}

const installExtensions = async () => {
  const installer = require("electron-devtools-assembler");
  const forceDownload = !!process.env.UPGRADE_EXTENSIONS;
  const extensions = ["REACT_DEVELOPER_TOOLS"];

  return installer
    .default(
      extensions.map((name) => installer[name]),
      forceDownload,
    )
    .catch(console.log);
};

const createWindow = async (pathsToOpen: string[]) => {
  if (isDebug) {
    await installExtensions();
  }

  const RESOURCES_PATH = app.isPackaged
    ? path.join(process.resourcesPath, "assets")
    : path.join(__dirname, "../../assets");

  const getAssetPath = (...assetPaths: string[]): string => {
    return path.join(RESOURCES_PATH, ...assetPaths);
  };

  mainWindow = new BrowserWindow({
    show: false,
    width: 1280,
    height: 768,
    minWidth: 640,
    minHeight: 400,
    icon: getAssetPath("icon.png"),
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false,
      sandbox: false,
      /* preload: app.isPackaged
        ? path.join(__dirname, "preload.js")
        : path.join(__dirname, "../../.erb/dll/preload.js"), */
    },
  });

  mainWindow.loadURL(resolveHtmlPath("index.html"));

  mainWindow.on("ready-to-show", () => {
    if (!mainWindow) {
      throw new Error('"mainWindow" is not defined');
    }
    if (process.env.START_MINIMIZED) {
      mainWindow.minimize();
    } else {
      mainWindow.show();
    }
  });

  mainWindow.on("show", () => {
    if (
      process.platform === "win32" &&
      process.env.NODE_ENV !== "development" &&
      process.argv.length > 1
    )
      mainWindow?.webContents.send("open-files", process.argv.slice(1));
    else if (process.platform === "darwin" && pathsToOpen.length > 0)
      mainWindow?.webContents.send("open-files", pathsToOpen);
  });

  mainWindow.on("closed", () => {
    mainWindow = null;
  });

  const menuBuilder = new MenuBuilder(mainWindow);
  menuBuilder.buildMenu();

  // Open urls in the user's browser
  mainWindow.webContents.setWindowOpenHandler((edata) => {
    shell.openExternal(edata.url);
    return {action: "deny"};
  });

  // Remove this if your app does not use auto updates
  // eslint-disable-next-line
  new AppUpdater();
};

/**
 * Add event listeners...
 */

app.on("window-all-closed", () => {
  // Respect the OSX convention of having the application in memory even
  // after all windows have been closed
  if (process.platform !== "darwin") {
    app.quit();
  }
});

const pathsToOpenAfterLaunch: string[] = [];

app.on("will-finish-launching", () => {
  app.on("open-file", (e, filePath) => {
    e.preventDefault();
    pathsToOpenAfterLaunch.push(filePath);
  });
});

app
  .whenReady()
  .then(() => {
    createWindow(pathsToOpenAfterLaunch);

    app.on("open-file", (e, filePath) => {
      e.preventDefault();
      if (mainWindow === null) createWindow([filePath]);
      else mainWindow.webContents.send("open-files", [filePath]);
    });
    app.on("activate", () => {
      // On macOS it's common to re-create a window in the app when the
      // dock icon is clicked and there are no other windows open.
      if (mainWindow === null) createWindow([]);
    });
  })
  .catch(console.log);
