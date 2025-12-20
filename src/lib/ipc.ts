// import {BrowserWindow, ipcMain, dialog, Menu, MenuItemConstructorOptions, app} from "electron";
// import settings from "electron-settings";
import {message, open} from "@tauri-apps/plugin-dialog";
import {path} from "@tauri-apps/api";
import {invoke} from "@tauri-apps/api/core";
import {listen, UnlistenFn} from "@tauri-apps/api/event";
import {SUPPORTED_TYPES} from "src/prototypes/constants/constants";
import {
  enableEditMenu,
  disableEditMenu,
  enablePlayMenu,
  disablePlayMenu,
} from "src/api/backend-wrapper";

export async function showOpenDialog() {
  // get the default path
  const projectRoot = await invoke<string | null>("get_project_root");
  const defaultPath = projectRoot ? await path.join(projectRoot, "samples/") : await path.homeDir();
  // const openDialogPath = ((await settings.get("openDialogPath")) as string) ?? defaultPath;

  // show the open dialog
  const files = await open({
    multiple: true,
    directory: false,
    filters: [{name: "Audio Files", extensions: SUPPORTED_TYPES}],
    title: "Select the audio files to be open",
    defaultPath,
    canCreateDirectories: false,
  });

  if (files && files.length > 0) {
    // find the common directory of the filepaths
    let commonDir = await path.resolve(files[0]);
    for (const filePath of files) {
      const resolved = await path.resolve(filePath);
      let newCommon = commonDir;
      while (!resolved.startsWith(newCommon)) {
        newCommon = await path.dirname(newCommon);
      }
      commonDir = newCommon;
    }

    // save the common directory to settings
    // await settings.set("openDialogPath", commonDir);
  }

  return files;
}

export function addAppRenderedListener(pathsToOpen: string[]) {
  /* ipcMain.once("app-rendered", (event) => {
    if (
      process.platform === "win32" &&
      process.env.NODE_ENV !== "development" &&
      process.argv.length > 1
    )
      event.reply("open-files", process.argv.slice(1));
    else if (process.platform === "darwin" && pathsToOpen.length > 0)
      event.reply("open-files", pathsToOpen);
  }); */
}

export function getOpenTracksHandler(
  callback: (files: string[]) => void | Promise<void>,
): () => Promise<void> {
  return async () => {
    await Promise.all([disablePlayMenu(), enableEditMenu()]);
    const files = await showOpenDialog();
    await Promise.all([enablePlayMenu(), disableEditMenu()]);
    if (files && files.length > 0) callback(files);
  };
}

const numFilesLabel = (numFiles: number) => (numFiles >= 5 ? ` (${numFiles} files)` : ``);
const joinManyPaths = (paths: string[]) => {
  // join with newline if less than 5 elements, else show first 2 elems + ellipse + the last elem
  return paths.length < 5
    ? paths.join("\n")
    : `${paths.slice(2).join("\n")}\n...\n${paths[paths.length - 1]}`;
};

export async function showFileOpenErrorMsg(unsupportedPaths: string[], invalidPaths: string[]) {
  const msgUnsupported = unsupportedPaths.length
    ? `-- Not Supported Type${numFilesLabel(unsupportedPaths.length)} --\n` +
      `${joinManyPaths(unsupportedPaths)}\n\n`
    : "";
  const msgInvalid = invalidPaths.length
    ? `-- Not Valid Format${numFilesLabel(invalidPaths.length)} --\n` +
      `${joinManyPaths(invalidPaths)}\n\n`
    : "";
  await message(
    "The following files could not be opened\n\n" +
      `${msgUnsupported}` +
      `${msgInvalid}` +
      "Please ensure that the file properties are correct and that it is a supported file type.\n\n" +
      `Only files with the following extensions are allowed:\n  ${SUPPORTED_TYPES.join(", ")}`,
    {title: "File Open Error", kind: "error"},
  );
}

export async function listenMenuOpenAudioTracks(handler: () => Promise<void>): Promise<UnlistenFn> {
  return listen("open-audio-tracks", handler);
}

export async function listenMenuEditDelete(
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen("edit-delete", handler);
}

export async function listenMenuEditSelectAll(
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen("edit-select-all", handler);
}

export async function listenEditMenuEvents() {
  const promiseUnlistenDelete = listenMenuEditDelete(() => {
    const activeElement = document.activeElement;
    if (activeElement instanceof HTMLInputElement || activeElement instanceof HTMLTextAreaElement) {
      if (activeElement.selectionStart === null || activeElement.selectionEnd === null) return;
      const text = activeElement.value;
      activeElement.value =
        text.slice(0, activeElement.selectionStart) + text.slice(activeElement.selectionEnd);
    }
  });
  const promiseUnlistenSelectAll = listenMenuEditSelectAll(() => {
    const activeElement = document.activeElement;
    if (activeElement instanceof HTMLInputElement || activeElement instanceof HTMLTextAreaElement) {
      activeElement.select();
    }
  });
  return Promise.all([promiseUnlistenDelete, promiseUnlistenSelectAll]);
}

export async function listenFreqZoomIn(handler: () => void | Promise<void>): Promise<UnlistenFn> {
  return listen("freq-zoom-in", handler);
}

export async function listenFreqZoomOut(handler: () => void | Promise<void>): Promise<UnlistenFn> {
  return listen("freq-zoom-out", handler);
}

export async function listenTimeZoomIn(handler: () => void | Promise<void>): Promise<UnlistenFn> {
  return listen("time-zoom-in", handler);
}

export async function listenTimeZoomOut(handler: () => void | Promise<void>): Promise<UnlistenFn> {
  return listen("time-zoom-out", handler);
}

export async function listenMenuRemoveSelectedTracks(
  handler: () => Promise<void>,
): Promise<UnlistenFn> {
  return listen("remove-selected-tracks", handler);
}

export async function listenMenuSelectAllTracks(
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen("select-all-tracks", handler);
}

export async function listenTogglePlay(handler: () => void | Promise<void>): Promise<UnlistenFn> {
  return listen("toggle-play", handler);
}

export type JumpPlayerMode = "fast-forward" | "rewind" | "fast-forward-big" | "rewind-big";

export async function listenJumpPlayer(
  handler: (mode: JumpPlayerMode) => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen<JumpPlayerMode>("jump-player", (event) => {
    handler(event.payload);
  });
}

export async function listenRewindToFront(
  handler: () => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen("rewind-to-front", handler);
}

export default function addIPCListeners() {
  // ipcMain.on("set-setting", (_, key, value) => settings.set(key, value));
  /* ipcMain.on("show-open-dialog", async (event) => {
    event.reply("remove-global-focusout-listener");
    mutateEditMenu((item) => {
      item.enabled = true;
    });
    const selectAllTracksMenu = Menu.getApplicationMenu()?.getMenuItemById("select-all-tracks");
    if (selectAllTracksMenu) selectAllTracksMenu.enabled = false;

    event.reply(
      "open-dialog-closed",
      await showOpenDialog(BrowserWindow.fromWebContents(event.sender)),
    );

    mutateEditMenu((item) => {
      item.enabled = false;
    });
    if (selectAllTracksMenu) selectAllTracksMenu.enabled = true;
    event.reply("add-global-focusout-listener");
  }); */
  /* ipcMain.on("show-track-context-menu", (event) => {
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
  }); */
  /* ipcMain.on("show-axis-context-menu", (event, axisKind, id) => {
    const template: MenuItemConstructorOptions[] = [];
    if (axisKind === "ampAxis") {
      template.push({
        ...labelAndSublabel("Edit Range", "Double Cick", "Double Click", 2),
        click: () => event.sender.send(`edit-${axisKind}-range-${id}`),
      });
    } else if (axisKind === "freqAxis") {
      template.push(
        {
          ...labelAndSublabel("Edit Upper Limit", "Double Click"),
          click: () => {
            event.sender.send(`edit-${axisKind}-range-${id}`, "max");
          },
        },
        {
          ...labelAndSublabel("Edit Lower Limit", "Double Click"),
          click: () => event.sender.send(`edit-${axisKind}-range-${id}`, "min"),
        },
      );
    }
    template.push({
      ...labelAndSublabel("Reset Range", "⌥+Click", "Alt+Click", 2),
      click: () => event.sender.send("reset-axis-range", axisKind),
    });
    const menu = Menu.buildFromTemplate(template);
    menu.popup({window: BrowserWindow.fromWebContents(event.sender) ?? undefined});
  }); */
  /* ipcMain.on("show-edit-context-menu", (event) => {
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
  }); */
  /* ipcMain
    .on("enable-edit-menu", () => {
      mutateEditMenu((item) => {
        item.enabled = true;
      });
      const selectAllTracksMenu = Menu.getApplicationMenu()?.getMenuItemById("select-all-tracks");
      if (selectAllTracksMenu) selectAllTracksMenu.enabled = false;
    })
    .on("disable-edit-menu", () => {
      mutateEditMenu((item) => {
        item.enabled = false;
      });
      const selectAllTracksMenu = Menu.getApplicationMenu()?.getMenuItemById("select-all-tracks");
      if (selectAllTracksMenu) selectAllTracksMenu.enabled = true;
    }); */
  /* ipcMain
     .on("enable-axis-zoom-menu", () => {
      const appMenu = Menu.getApplicationMenu();
      if (!appMenu) return;
      ["freq-zoom-in", "freq-zoom-out", "time-zoom-in", "time-zoom-out"].forEach((name) => {
        const menu = appMenu.getMenuItemById(name);
        if (menu) menu.enabled = true;
      });
    })
    .on("disable-axis-zoom-menu", () => {
      const appMenu = Menu.getApplicationMenu();
      if (!appMenu) return;
      ["freq-zoom-in", "freq-zoom-out", "time-zoom-in", "time-zoom-out"].forEach((name) => {
        const menu = appMenu.getMenuItemById(name);
        if (menu) menu.enabled = false;
      });
    }); */
  /* ipcMain
    .on("enable-remove-track-menu", () => {
      const removeTrackMenu = Menu.getApplicationMenu()?.getMenuItemById("remove-selected-tracks");
      if (removeTrackMenu) removeTrackMenu.enabled = true;
    })
    .on("disable-remove-track-menu", () => {
      const removeTrackMenu = Menu.getApplicationMenu()?.getMenuItemById("remove-selected-tracks");
      if (removeTrackMenu) removeTrackMenu.enabled = false;
    }); */
  /* ipcMain
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
    })
    .on("enable-toggle-play-menu", () => {
      const appMenu = Menu.getApplicationMenu();
      if (appMenu === null) return;
      const togglePlayMenu = appMenu.getMenuItemById("play");
      if (togglePlayMenu) togglePlayMenu.enabled = true;
      const togglePauseMenu = appMenu.getMenuItemById("pause");
      if (togglePauseMenu) togglePauseMenu.enabled = true;
    })
    .on("disable-toggle-play-menu", () => {
      const appMenu = Menu.getApplicationMenu();
      if (appMenu === null) return;
      const togglePlayMenu = appMenu.getMenuItemById("play");
      if (togglePlayMenu) togglePlayMenu.enabled = false;
      const togglePauseMenu = appMenu.getMenuItemById("pause");
      if (togglePauseMenu) togglePauseMenu.enabled = false;
    }); */
}
