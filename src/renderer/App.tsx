import React, {useCallback, useEffect, useRef} from "react";
import {ipcRenderer} from "electron";
import Control from "./prototypes/Control/Control";
import Overview from "./prototypes/Overview/Overview";
import SlideBar from "./prototypes/SlideBar/SlideBar";
import MainViewer from "./prototypes/MainViewer/MainViewer";
import {showElectronFileOpenErrorMsg} from "./lib/electron-sender";
import {SUPPORTED_MIME} from "./prototypes/constants";
import "./App.global.scss";
import useTracks from "./hooks/useTracks";
import useSelectedTracks from "./hooks/useSelectedTracks";

function App() {
  const {
    trackIds,
    erroredTrackIds,
    needRefreshTrackIdChArr,
    reloadTracks,
    refreshTracks,
    addTracks,
    removeTracks,
    ignoreError,
  } = useTracks();
  const {selectedTrackIds, selectTrack, selectTrackAfterAddTracks, selectTrackAfterRemoveTracks} =
    useSelectedTracks();

  const prevTrackIds = useRef<number[]>([]);

  const addDroppedFile = useCallback(
    async (e: DragEvent) => {
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
        reloadTracks(existingIds);
      }
      refreshTracks();
    },
    [addTracks, reloadTracks, refreshTracks],
  );

  const deleteSelectedTracks = useCallback(
    (e: KeyboardEvent) => {
      e.preventDefault();

      if (e.key === "Delete" || e.key === "Backspace") {
        if (selectedTrackIds.length) {
          removeTracks(selectedTrackIds);
          refreshTracks();
        }
      }
    },
    [selectedTrackIds, removeTracks, refreshTracks],
  );

  useEffect(() => {
    ipcRenderer.on("open-dialog-closed", async (_, file) => {
      if (!file.canceled) {
        const newPaths: string[] = file.filePaths;
        const unsupportedPaths: string[] = [];

        const {existingIds, invalidPaths} = await addTracks(newPaths);

        if (unsupportedPaths.length || invalidPaths.length) {
          showElectronFileOpenErrorMsg(unsupportedPaths, invalidPaths);
        }

        if (existingIds.length) {
          reloadTracks(existingIds);
        }
        refreshTracks();
      } else {
        console.log("file canceled: ", file.canceled);
      }
    });

    return () => {
      ipcRenderer.removeAllListeners("open-dialog-closed");
    };
  }, [addTracks, reloadTracks, refreshTracks]);

  useEffect(() => {
    ipcRenderer.on("delete-track", (_, targetTrackId) => {
      removeTracks([targetTrackId]);
      refreshTracks();
    });
    return () => {
      ipcRenderer.removeAllListeners("delete-track");
    };
  }, [removeTracks, refreshTracks]);

  useEffect(() => {
    document.addEventListener("keydown", deleteSelectedTracks);

    return () => {
      document.removeEventListener("keydown", deleteSelectedTracks);
    };
  }, [deleteSelectedTracks]);

  useEffect(() => {
    const prevTrackIdsCount = prevTrackIds.current.length;
    const currTrackIdsCount = trackIds.length;

    if (prevTrackIdsCount === currTrackIdsCount) {
      return;
    }

    if (prevTrackIdsCount < currTrackIdsCount) {
      selectTrackAfterAddTracks(prevTrackIds.current, trackIds);
    } else {
      selectTrackAfterRemoveTracks(prevTrackIds.current, trackIds);
    }

    prevTrackIds.current = trackIds;
  }, [trackIds, selectTrackAfterAddTracks, selectTrackAfterRemoveTracks]);

  return (
    <div className="App">
      <div className="row-fixed control">
        <Control />
      </div>
      <div className="row-fixed overview">
        <Overview />
        <SlideBar />
      </div>
      <MainViewer
        trackIds={trackIds}
        erroredTrackIds={erroredTrackIds}
        needRefreshTrackIdChArr={needRefreshTrackIdChArr}
        selectedTrackIds={selectedTrackIds}
        addDroppedFile={addDroppedFile}
        ignoreError={ignoreError}
        refreshTracks={refreshTracks}
        reloadTracks={reloadTracks}
        removeTracks={removeTracks}
        selectTrack={selectTrack}
      />
    </div>
  );
}

export default App;
