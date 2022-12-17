import React, {useCallback, useEffect, useRef} from "react";
import {ipcRenderer} from "electron";
import Control from "./prototypes/Control/Control";
import Overview from "./prototypes/Overview/Overview";
import SlideBar from "./prototypes/SlideBar/SlideBar";
import MainViewer from "./prototypes/MainViewer/MainViewer";
import {SUPPORTED_TYPES, SUPPORTED_MIME} from "./prototypes/constants";
import "./App.global.scss";
import styles from "./prototypes/MainViewer/MainViewer.scss";
import useTracks from "./hooks/useTracks";

function App() {
  const selectedIdsRef = useRef<number[]>([]);
  const nextSelectedIndexRef = useRef<number | null>(null);

  const {
    trackIds,
    erroredList,
    refreshList,
    reloadTracks,
    refreshTracks,
    addTracks,
    removeTracks,
    ignoreError,
  } = useTracks();

  function showOpenDialog() {
    ipcRenderer.send("show-open-dialog", SUPPORTED_TYPES);
  }

  function addDroppedFile(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();

    const newPaths: string[] = [];
    const unsupportedPaths: string[] = [];

    if (!e?.dataTransfer?.files) {
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

    const {existingIds, invalidPaths} = addTracks(newPaths);
    if (unsupportedPaths.length || invalidPaths.length) {
      ipcRenderer.send("show-file-open-err-msg", unsupportedPaths, invalidPaths, SUPPORTED_TYPES);
    }

    if (existingIds.length) {
      reloadTracks(existingIds);
    }
    refreshTracks();
  }

  const assignSelectedClass = (selectedIds: number[]) => {
    const targets = document.querySelectorAll(".js-track-left");
    targets.forEach((target) => {
      if (selectedIds.includes(Number(target.getAttribute("id")))) {
        target.classList.add(styles.selected);
      } else {
        target.classList.remove(styles.selected);
      }
    });
  };
  const selectTrack = (e: React.MouseEvent) => {
    e.preventDefault();

    const targetClassList = e.currentTarget.classList;
    const targetTrackId = Number(e.currentTarget.getAttribute("id"));

    if (!targetClassList.contains(styles.selected)) {
      selectedIdsRef.current = [targetTrackId];
      assignSelectedClass(selectedIdsRef.current);
    }
  };
  const deleteSelectedTracks = useCallback(
    (e: KeyboardEvent) => {
      e.preventDefault();

      if (e.key === "Delete" || e.key === "Backspace") {
        if (selectedIdsRef.current.length) {
          removeTracks(selectedIdsRef.current);
          refreshTracks();
        }
      }
    },
    [removeTracks, refreshTracks],
  );

  const showTrackContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    const targetTrackId = Number(e.currentTarget.getAttribute("id"));
    ipcRenderer.send("show-track-context-menu", targetTrackId);
  };

  useEffect(() => {
    ipcRenderer.on("open-dialog-closed", (_, file) => {
      if (!file.canceled) {
        const newPaths: string[] = file.filePaths;
        const unsupportedPaths: string[] = [];

        const {existingIds, invalidPaths} = addTracks(newPaths);

        if (unsupportedPaths.length || invalidPaths.length) {
          ipcRenderer.send(
            "show-file-open-err-msg",
            unsupportedPaths,
            invalidPaths,
            SUPPORTED_TYPES,
          );
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
    const trackCount = trackIds.length;
    if (!trackCount) {
      selectedIdsRef.current = [];
    } else if (nextSelectedIndexRef.current && nextSelectedIndexRef.current < trackCount) {
      selectedIdsRef.current = [trackIds[nextSelectedIndexRef.current]];
    } else {
      selectedIdsRef.current = [trackIds[trackCount - 1]];
    }
    assignSelectedClass(selectedIdsRef.current);
    nextSelectedIndexRef.current = null;
  }, [trackIds]);

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
        erroredList={erroredList}
        refreshList={refreshList}
        trackIds={trackIds}
        addDroppedFile={addDroppedFile}
        ignoreError={ignoreError}
        refreshTracks={refreshTracks}
        reloadTracks={reloadTracks}
        removeTracks={removeTracks}
        showOpenDialog={showOpenDialog}
        selectTrack={selectTrack}
        showTrackContextMenu={showTrackContextMenu}
      />
    </div>
  );
}

export default App;
