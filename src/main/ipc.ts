import {BrowserWindow, ipcMain, dialog, Menu, MenuItemConstructorOptions, app} from "electron";
import os from "os";
import path from "path";
import {SUPPORTED_TYPES} from "./constants";

function labelAndSublabel(
  label: string,
  darwinAccelerator: string,
  otherAccelerator?: string,
  nTabs: number = 1,
) {
  let labelWithAcc = label;
  const sublabel = otherAccelerator !== undefined ? otherAccelerator : darwinAccelerator;
  if (os.platform() === "darwin") labelWithAcc += `${"\t".repeat(nTabs)}(${darwinAccelerator})`;
  else
    labelWithAcc += " ".repeat(
      Math.max(Math.round(sublabel.length * 1.5 - labelWithAcc.length), 0),
    );
  return {label: labelWithAcc, sublabel};
}

export function showOpenDialog() {
  const defaultPath = app.isPackaged ? app.getPath("home") : path.join(__dirname, "../../samples/");

  return dialog.showOpenDialog({
    title: "Select the audio files to be open",
    defaultPath,
    filters: [
      {
        name: "Audio Files",
        extensions: SUPPORTED_TYPES,
      },
    ],
    properties: ["openFile", "multiSelections"],
  });
}

export default function addIPCListeners() {
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
    const menu = Menu.buildFromTemplate([
      {
        ...labelAndSublabel("Remove Selected Tracks", "⌫ | ⌦", "Del | Backspace"),
        click: () => event.sender.send("remove-selected-tracks"),
      },
      {
        ...labelAndSublabel("Select All Tracks", "(⌘+A)", "Ctrl+A", 3),
        click: () => event.sender.send("select-all-tracks"),
      },
    ]);
    menu.popup({window: BrowserWindow.fromWebContents(event.sender) ?? undefined});
  });

  ipcMain.on("show-axis-context-menu", (event, axisKind) => {
    const template: MenuItemConstructorOptions[] = [];
    if (axisKind === "ampAxis") {
      template.push({
        ...labelAndSublabel("Edit Range", "Double Cick", "Double Click", 2),
        click: () => event.sender.send("edit-axis-range", axisKind),
      });
    } else if (axisKind === "freqAxis") {
      template.push(
        {
          ...labelAndSublabel("Edit Upper Limit", "Double Click"),
          click: () => {
            event.sender.send("edit-axis-range", axisKind, "max");
          },
        },
        {
          ...labelAndSublabel("Edit Lower Limit", "Double Click"),
          click: () => event.sender.send("edit-axis-range", axisKind, "min"),
        },
      );
    }
    template.push({
      ...labelAndSublabel("Reset Range", "⌥+Click", "Alt+Click", 2),
      click: () => event.sender.send("reset-axis-range", axisKind),
    });
    const menu = Menu.buildFromTemplate(template);
    menu.popup({window: BrowserWindow.fromWebContents(event.sender) ?? undefined});
  });

  ipcMain.on("show-edit-context-menu", (event) => {
    const menu = Menu.buildFromTemplate([
      {role: "undo"},
      {role: "redo"},
      {type: "separator"},
      {role: "cut"},
      {role: "copy"},
      {role: "paste"},
      {type: "separator"},
      {role: "selectAll"},
    ]);
    menu.popup({window: BrowserWindow.fromWebContents(event.sender) ?? undefined});
  });

  ipcMain
    .on("enable-edit-menu", () => {
      const appMenu = Menu.getApplicationMenu();
      if (!appMenu) return;
      const editMenu = appMenu.getMenuItemById("edit-menu");
      if (editMenu) {
        editMenu.submenu?.items.forEach((item) => {
          item.enabled = true;
        });
      }
      const selectAllTracksMenu = appMenu.getMenuItemById("select-all-tracks");
      if (selectAllTracksMenu) selectAllTracksMenu.enabled = false;
    })
    .on("disable-edit-menu", () => {
      const appMenu = Menu.getApplicationMenu();
      if (!appMenu) return;
      const editMenu = appMenu.getMenuItemById("edit-menu");
      if (editMenu) {
        editMenu.submenu?.items.forEach((item) => {
          item.enabled = false;
        });
      }
      const selectAllTracksMenu = appMenu.getMenuItemById("select-all-tracks");
      if (selectAllTracksMenu) selectAllTracksMenu.enabled = true;
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
}