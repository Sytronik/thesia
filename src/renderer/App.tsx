import {MemoryRouter as Router, Routes, Route} from "react-router-dom";
import {useEffect, useRef} from "react";
import useEvent from "react-use-event-hook";
import {ipcRenderer} from "electron";
import {DndProvider} from "react-dnd";
import {HTML5Backend} from "react-dnd-html5-backend";
import {UserSettings} from "backend";
import Control from "./prototypes/Control/Control";
import MainViewer from "./prototypes/MainViewer/MainViewer";
import PlayerControl from "./prototypes/PlayerControl/PlayerControl";
import {
  addGlobalFocusInListener,
  addGlobalFocusOutListener,
  changeMenuDepsOnTrackExistence,
  notifyAppRendered,
  removeGlobalFocusInListener,
  removeGlobalFocusOutListener,
  showEditContextMenuIfEditableNode,
  showElectronFileOpenErrorMsg,
} from "./lib/ipc-sender";
import {SUPPORTED_MIME} from "../main/constants";
import useTracks from "./hooks/useTracks";
import useSelectedTracks from "./hooks/useSelectedTracks";
import {DevicePixelRatioProvider} from "./contexts";
import usePlayer from "./hooks/usePlayer";
import "./App.scss";

type AppProps = {userSettings: UserSettings};

function MyApp({userSettings}: AppProps) {
  const {
    trackIds,
    hiddenTrackIds,
    erroredTrackIds,
    trackIdChMap,
    isLoading,
    needRefreshTrackIdChArr,
    maxTrackSec,
    maxTrackHz,
    specSetting,
    blend,
    dBRange,
    commonNormalize,
    commonGuardClipping,
    reloadTracks,
    refreshTracks,
    addTracks,
    removeTracks,
    hideTracks,
    changeTrackOrder,
    showHiddenTracks,
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
    selectionIsAdded,
    selectTrack,
    selectAllTracks,
    selectTrackAfterAddTracks,
    selectTrackAfterRemoveTracks,
  } = useSelectedTracks();

  const player = usePlayer(
    selectedTrackIds.length > 0 &&
      !erroredTrackIds.includes(selectedTrackIds[selectedTrackIds.length - 1])
      ? selectedTrackIds[selectedTrackIds.length - 1]
      : -1,
    maxTrackSec,
  );

  const prevTrackIds = useRef<number[]>([]); // includes hidden tracks

  const addDroppedFile = useEvent(async (item: {files: File[]}, index: number) => {
    const newPaths: string[] = [];
    const unsupportedPaths: string[] = [];

    if (item.files.length === 0) {
      console.error("no file exists in dropzone");
      return;
    }

    const droppedFiles = Array.from(item.files);

    droppedFiles.forEach((file: File) => {
      if (SUPPORTED_MIME.includes(file.type)) {
        newPaths.push(file.path);
      } else {
        unsupportedPaths.push(file.path);
      }
    });

    const {existingIds, invalidPaths} = await addTracks(newPaths, index);

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
    addGlobalFocusInListener();
    addGlobalFocusOutListener();
    return () => {
      removeGlobalFocusInListener();
      removeGlobalFocusOutListener();
    };
  }, []);

  useEffect(() => {
    ipcRenderer.on("add-global-focusout-listener", addGlobalFocusOutListener);
    ipcRenderer.on("remove-global-focusout-listener", removeGlobalFocusOutListener);
    return () => {
      ipcRenderer.removeAllListeners("add-global-focusout-listener");
      ipcRenderer.removeAllListeners("remove-global-focusout-listener");
    };
  });

  useEffect(() => {
    const prevTrackIdsCount = prevTrackIds.current.length;
    const currTrackIdsCount = trackIds.length + hiddenTrackIds.length;

    if (prevTrackIdsCount === currTrackIdsCount) return;

    changeMenuDepsOnTrackExistence(currTrackIdsCount > 0);

    if (prevTrackIdsCount < currTrackIdsCount) {
      selectTrackAfterAddTracks(prevTrackIds.current, trackIds);
    } else {
      selectTrackAfterRemoveTracks(prevTrackIds.current, trackIds);
    }

    prevTrackIds.current = trackIds.concat(hiddenTrackIds);
  }, [trackIds, hiddenTrackIds, selectTrackAfterAddTracks, selectTrackAfterRemoveTracks]);

  return (
    <div id="App" className="App">
      <DndProvider backend={HTML5Backend}>
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
            isLoading={isLoading}
          />
          <DevicePixelRatioProvider>
            <MainViewer
              trackIds={trackIds}
              erroredTrackIds={erroredTrackIds}
              selectedTrackIds={selectedTrackIds}
              selectionIsAdded={selectionIsAdded}
              trackIdChMap={trackIdChMap}
              isLoading={isLoading}
              needRefreshTrackIdChArr={needRefreshTrackIdChArr}
              maxTrackSec={maxTrackSec}
              maxTrackHz={maxTrackHz}
              blend={blend}
              player={player}
              addDroppedFile={addDroppedFile}
              ignoreError={ignoreError}
              refreshTracks={refreshTracks}
              reloadTracks={reloadTracks}
              removeTracks={removeTracks}
              hideTracks={hideTracks}
              changeTrackOrder={changeTrackOrder}
              showHiddenTracks={showHiddenTracks}
              selectTrack={selectTrack}
              selectAllTracks={selectAllTracks}
              finishRefreshTracks={finishRefreshTracks}
            />
          </DevicePixelRatioProvider>
        </div>
      </DndProvider>
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
