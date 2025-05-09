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
import {app, BrowserWindow, shell} from "electron";
import {autoUpdater} from "electron-updater";
import log from "electron-log";
import settings from "electron-settings";
import {installExtension, REACT_DEVELOPER_TOOLS} from "electron-extension-installer";
import MenuBuilder from "./menu";
import {resolveHtmlPath} from "./util";
import addIPCListeners, {addAppRenderedListener} from "./ipc";

class AppUpdater {
  constructor() {
    log.transports.file.level = "info";
    autoUpdater.logger = log;
    autoUpdater.checkForUpdatesAndNotify();
  }
}

let mainWindow: BrowserWindow | null = null;

addIPCListeners();
settings.configure({});

if (process.env.NODE_ENV === "production") {
  const sourceMapSupport = require("source-map-support");
  sourceMapSupport.install();
}

const isDebug = process.env.NODE_ENV === "development" || process.env.DEBUG_PROD === "true";

if (isDebug) {
  require("electron-debug")();
  console.log(`setting file: "${settings.file()}"`);
}

const installExtensions = async () => {
  try {
    await installExtension(REACT_DEVELOPER_TOOLS, {
      loadExtensionOptions: {
        allowFileAccess: true,
      },
    });
  } catch (err) {
    console.log(err);
  }
};

const createWindow = async (pathsToOpen: string[]) => {
  if (isDebug) await installExtensions();

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

  addAppRenderedListener(pathsToOpen);

  mainWindow.on("ready-to-show", () => {
    if (!mainWindow) throw new Error('"mainWindow" is not defined');

    mainWindow.webContents.send("render-with-settings", settings.getSync());
    if (process.env.START_MINIMIZED) mainWindow.minimize();
    else mainWindow.show();
  });

  // This is needed to reload the window on Windows.
  // contents.reload in menu.ts randomly fails after forcefullyCrashRenderer.
  // (refer to killAndReload in menu.ts)
  mainWindow.webContents.on("render-process-gone", () => {
    console.log("renderer process gone. reloading ...");
    mainWindow?.webContents.reload();
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
  if (process.platform !== "darwin") app.quit();
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
