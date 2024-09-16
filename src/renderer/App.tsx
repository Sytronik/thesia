import {MemoryRouter as Router, Routes, Route} from "react-router-dom";
import React, {useEffect, useRef} from "react";
import useEvent from "react-use-event-hook";
import {ipcRenderer} from "electron";
import Control from "./prototypes/Control/Control";
import MainViewer from "./prototypes/MainViewer/MainViewer";
import PlayerControl from "./prototypes/PlayerControl/PlayerControl";
import {
  changeMenuDepsOnTrackExistence,
  disableEditMenu,
  enableEditMenu,
  notifyAppRendered,
  showEditContextMenu,
  showElectronFileOpenErrorMsg,
} from "./lib/ipc-sender";
import {SUPPORTED_MIME} from "./prototypes/constants/tracks";
import "./App.scss";
import useTracks from "./hooks/useTracks";
import useSelectedTracks from "./hooks/useSelectedTracks";
import {DevicePixelRatioProvider} from "./contexts";
import usePlayer from "./hooks/usePlayer";

type AppProps = {userSettings: UserSettings};

const callDifferentFuncIfEditableNode = (
  node: HTMLElement | null,
  funcForEditable: () => void,
  funcForNonEditable?: () => void,
) => {
  let ancestor = node;
  let isEditable = false;
  while (ancestor) {
    if (ancestor.nodeName.match(/^(input|textarea)$/i) || ancestor.isContentEditable) {
      funcForEditable();
      isEditable = true;
      break;
    }
    if (ancestor.parentNode === null) break;
    ancestor = ancestor.parentNode as HTMLElement;
  }
  if (!isEditable && funcForNonEditable) funcForNonEditable();
};

const showEditContextMenuIfEditableNode = (e: MouseEvent) => {
  e.preventDefault();
  e.stopPropagation();
  callDifferentFuncIfEditableNode(e.target as HTMLElement | null, showEditContextMenu);
};

const changeEditMenuForFocusIn = (e: FocusEvent) => {
  callDifferentFuncIfEditableNode(e.target as HTMLElement | null, enableEditMenu, disableEditMenu);
};

const changeEditMenuForFocusOut = (e: FocusEvent) => {
  callDifferentFuncIfEditableNode(
    e.relatedTarget as HTMLElement | null,
    enableEditMenu,
    disableEditMenu,
  );
};

function MyApp({userSettings}: AppProps) {
  const {
    trackIds,
    erroredTrackIds,
    trackIdChMap,
    needRefreshTrackIdChArr,
    maxTrackSec,
    specSetting,
    blend,
    dBRange,
    commonNormalize,
    commonGuardClipping,
    reloadTracks,
    refreshTracks,
    addTracks,
    removeTracks,
    ignoreError,
    setSpecSetting,
    setBlend,
    setdBRange,
    setCommonNormalize,
    setCommonGuardClipping,
    finishRefreshTracks,
  } = useTracks(userSettings);

  const {
    selectedTrackIds,
    selectTrack,
    selectAllTracks,
    selectTrackAfterAddTracks,
    selectTrackAfterRemoveTracks,
  } = useSelectedTracks();
  const player = usePlayer(
    selectedTrackIds.length > 0 ? selectedTrackIds[selectedTrackIds.length - 1] : -1,
  );

  const prevTrackIds = useRef<number[]>([]);

  const addDroppedFile = useEvent(async (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const newPaths: string[] = [];
    const unsupportedPaths: string[] = [];

    if (!e?.dataTransfer?.files) {
      console.error("no file exists in dropzone");
      return;
    }

    const droppedFiles = Array.from(e.dataTransfer.files);

    droppedFiles.forEach((file: File) => {
      if (SUPPORTED_MIME.includes(file.type)) {
        newPaths.push(file.path);
      } else {
        unsupportedPaths.push(file.path);
      }
    });

    const {existingIds, invalidPaths} = await addTracks(newPaths);

    if (unsupportedPaths.length || invalidPaths.length) {
      showElectronFileOpenErrorMsg(unsupportedPaths, invalidPaths);
    }
    if (existingIds.length) {
      await reloadTracks(existingIds);
    }
    await refreshTracks();
  });

  const openFiles = useEvent(async (filePaths: string[]) => {
    const unsupportedPaths: string[] = [];

    const {existingIds, invalidPaths} = await addTracks(filePaths);

    if (unsupportedPaths.length || invalidPaths.length) {
      showElectronFileOpenErrorMsg(unsupportedPaths, invalidPaths);
    }

    if (existingIds.length) {
      await reloadTracks(existingIds);
    }
    await refreshTracks();
  });

  useEffect(() => {
    ipcRenderer.on("open-files", async (_, filePaths) => openFiles(filePaths));
    return () => {
      ipcRenderer.removeAllListeners("open-files");
    };
  }, [openFiles]);

  useEffect(() => {
    ipcRenderer.on(
      "open-dialog-closed",
      async (_, dialogResult: Electron.OpenDialogReturnValue) => {
        if (!dialogResult.canceled) openFiles(dialogResult.filePaths);
      },
    );

    return () => {
      ipcRenderer.removeAllListeners("open-dialog-closed");
    };
  }, [openFiles]);

  const removeSelectedTracks = useEvent(async () => {
    if (selectedTrackIds.length === 0) return;
    removeTracks(selectedTrackIds);
    await refreshTracks();
  });

  useEffect(() => {
    ipcRenderer.on("remove-selected-tracks", removeSelectedTracks);
    return () => {
      ipcRenderer.removeAllListeners("remove-selected-tracks");
    };
  }, [removeSelectedTracks]);

  useEffect(() => {
    document.body.addEventListener("contextmenu", showEditContextMenuIfEditableNode);
    return () => {
      document.body.removeEventListener("contextmenu", showEditContextMenuIfEditableNode);
    };
  }, []);

  useEffect(() => {
    document.body.addEventListener("focusin", changeEditMenuForFocusIn);
    document.body.addEventListener("focusout", changeEditMenuForFocusOut);
    return () => {
      document.body.removeEventListener("focusin", changeEditMenuForFocusIn);
      document.body.removeEventListener("focusout", changeEditMenuForFocusOut);
    };
  }, []);

  useEffect(() => {
    const prevTrackIdsCount = prevTrackIds.current.length;
    const currTrackIdsCount = trackIds.length;

    if (prevTrackIdsCount === currTrackIdsCount) return;

    changeMenuDepsOnTrackExistence(currTrackIdsCount > 0);

    if (prevTrackIdsCount < currTrackIdsCount) {
      selectTrackAfterAddTracks(prevTrackIds.current, trackIds);
    } else {
      selectTrackAfterRemoveTracks(prevTrackIds.current, trackIds);
    }

    prevTrackIds.current = trackIds;
  }, [trackIds, selectTrackAfterAddTracks, selectTrackAfterRemoveTracks]);

  return (
    <div id="App" className="App">
      <PlayerControl player={player} isTrackEmpty={trackIds.length === 0} />
      <div className="flex-container-row flex-item-auto">
        <Control
          specSetting={specSetting}
          setSpecSetting={setSpecSetting}
          blend={blend}
          setBlend={setBlend}
          dBRange={dBRange}
          setdBRange={setdBRange}
          commonGuardClipping={commonGuardClipping}
          setCommonGuardClipping={setCommonGuardClipping}
          commonNormalize={commonNormalize}
          setCommonNormalize={setCommonNormalize}
        />
        <DevicePixelRatioProvider>
          <MainViewer
            trackIds={trackIds}
            erroredTrackIds={erroredTrackIds}
            selectedTrackIds={selectedTrackIds}
            trackIdChMap={trackIdChMap}
            needRefreshTrackIdChArr={needRefreshTrackIdChArr}
            maxTrackSec={maxTrackSec}
            blend={blend}
            player={player}
            addDroppedFile={addDroppedFile}
            ignoreError={ignoreError}
            refreshTracks={refreshTracks}
            reloadTracks={reloadTracks}
            removeTracks={removeTracks}
            selectTrack={selectTrack}
            selectAllTracks={selectAllTracks}
            finishRefreshTracks={finishRefreshTracks}
          />
        </DevicePixelRatioProvider>
      </div>
    </div>
  );
}

export default function App({userSettings}: AppProps) {
  useEffect(notifyAppRendered, []);

  return (
    <Router>
      <Routes>
        <Route path="/" element={<MyApp userSettings={userSettings} />} />
      </Routes>
    </Router>
  );
}
