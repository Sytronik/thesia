import React, {useEffect, useRef, useState, useCallback} from "react";
import {ipcRenderer} from "electron";
import Control from "./prototypes/Control/Control";
import Overview from "./prototypes/Overview/Overview";
import SlideBar from "./prototypes/SlideBar/SlideBar";
import MainViewer from "./prototypes/MainViewer/MainViewer";
import {SUPPORTED_TYPES, SUPPORTED_MIME} from "./prototypes/constants";
import "./App.global.scss";
import styles from "./prototypes/MainViewer/MainViewer.scss";
import useTracks from "./hooks/useTracks";

const backend = require("backend");

function App() {
  const selectedIdsRef = useRef([]);
  const nextSelectedIndexRef = useRef(null);

  const {trackIds, erroredList, refreshList, reloadTracks, addTracks, removeTracks, ignoreError} =
    useTracks();

  function showOpenDialog() {
    ipcRenderer.send("show-open-dialog", SUPPORTED_TYPES);
  }

  function addDroppedFile(e) {
    e.preventDefault();
    e.stopPropagation();

    const newPaths = [];
    const unsupportedPaths = [];

    e.dataTransfer.files.forEach((file) => {
      if (SUPPORTED_MIME.includes(file.type)) {
        newPaths.push(file.path);
      } else {
        unsupportedPaths.push(file.path);
      }
    });
    addTracks(newPaths, unsupportedPaths);
  }

  const assignSelectedClass = (selectedIds) => {
    const targets = document.querySelectorAll(".js-track-left");
    targets.forEach((target) => {
      if (selectedIds.includes(Number(target.getAttribute("id")))) {
        target.classList.add(styles.selected);
      } else {
        target.classList.remove(styles.selected);
      }
    });
  };
  const selectTrack = (e) => {
    e.preventDefault();

    const targetClassList = e.currentTarget.classList;
    const targetTrackId = Number(e.currentTarget.getAttribute("id"));

    if (!targetClassList.contains(styles.selected)) {
      selectedIdsRef.current = [targetTrackId];
      assignSelectedClass(selectedIdsRef.current);
    }
  };
  const deleteSelectedTracks = (e) => {
    e.preventDefault();

    if (e.key === "Delete" || e.key === "Backspace") {
      if (selectedIdsRef.current.length) {
        removeTracks(selectedIdsRef.current);
      }
    }
  };

  const showTrackContextMenu = (e) => {
    e.preventDefault();
    const targetTrackId = Number(e.currentTarget.getAttribute("id"));
    ipcRenderer.send("show-track-context-menu", targetTrackId);
  };

  useEffect(() => {
    ipcRenderer.on("open-dialog-closed", (event, file) => {
      if (!file.canceled) {
        const newPaths = file.filePaths;
        const unsupportedPaths = [];

        addTracks(newPaths, unsupportedPaths);
      } else {
        console.log("file canceled: ", file.canceled);
      }
    });

    return () => {
      ipcRenderer.removeAllListeners("open-dialog-closed");
    };
  }, [addTracks]);

  useEffect(() => {
    ipcRenderer.on("delete-track", (e, targetTrackId) => {
      removeTracks([targetTrackId]);
    });
    return () => {
      ipcRenderer.removeAllListeners("delete-track");
    };
  }, [removeTracks]);

  useEffect(() => {
    document.addEventListener("keydown", deleteSelectedTracks);

    return () => {
      document.removeEventListener("keydown", deleteSelectedTracks);
    };
  });

  useEffect(() => {
    const trackCount = trackIds.length;
    if (!trackCount) {
      selectedIdsRef.current = [];
    } else if (nextSelectedIndexRef.current < trackCount) {
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
