import React, {useEffect, useRef, useState, useCallback} from "react";
import {ipcRenderer} from "electron";
import Control from "./prototypes/Control/Control";
import Overview from "./prototypes/Overview/Overview";
import SlideBar from "./prototypes/SlideBar/SlideBar";
import MainViewer from "./prototypes/MainViewer/MainViewer";
import PROPERTY from "./prototypes/Property";
import "./App.global.scss";
import styles from "./prototypes/MainViewer/MainViewer.scss";

const backend = require("backend");

const {SUPPORTED_TYPES} = PROPERTY;
const SUPPORTED_MIME = SUPPORTED_TYPES.map((subtype) => `audio/${subtype}`);

function App() {
  const waitingIdsRef = useRef([]);
  const selectedIdsRef = useRef([]);
  const nextSelectedIndexRef = useRef(null);

  const [trackIds, setTrackIds] = useState([]);
  const [erroredList, setErroredList] = useState([]);
  const [refreshList, setRefreshList] = useState([]);

  async function reloadTracks(selectedIds) {
    const reloadedIds = backend.reloadTracks(selectedIds);

    setErroredList(selectedIds.filter((id) => !reloadedIds.includes(id)));
    setRefreshList(backend.applyTrackListChanges());
  }

  const addTracks = useCallback(
    (newPaths, unsupportedPaths) => {
      try {
        const newIds = [];
        const existingIds = [];
        let invalidIds = [];
        let invalidPaths = [];

        newPaths.forEach((path, i, newPaths) => {
          const id = backend.findIDbyPath(path);
          if (id !== -1) {
            newPaths.splice(i, 1);
            existingIds.push(id);
          }
        });

        if (newPaths.length) {
          for (let i = 0; i < newPaths.length; i += 1) {
            if (waitingIdsRef.current.length) {
              newIds.push(waitingIdsRef.current.shift());
            } else {
              newIds.push(trackIds.length + i);
            }
          }

          nextSelectedIndexRef.current = trackIds.length;
          const addedIds = backend.addTracks(newIds, newPaths);
          setTrackIds((trackIds) => trackIds.concat(addedIds));

          if (newIds.length !== addedIds.length) {
            invalidIds = newIds.filter((id) => !addedIds.includes(id));
            invalidPaths = invalidIds.map((id) => newPaths[newIds.indexOf(id)]);

            waitingIdsRef.current = waitingIdsRef.current.concat(invalidIds);
            if (waitingIdsRef.current.length > 1) {
              waitingIdsRef.current.sort((a, b) => a - b);
            }
          }
          if (unsupportedPaths.length || invalidPaths.length) {
            ipcRenderer.send(
              "show-file-open-err-msg",
              unsupportedPaths,
              invalidPaths,
              SUPPORTED_TYPES,
            );
          }
        }

        if (existingIds.length) {
          reloadTracks(existingIds);
        } else {
          setRefreshList(backend.applyTrackListChanges());
        }
      } catch (err) {
        console.log(err);
        alert("File upload error");
      }
    },
    [trackIds],
  );

  const ignoreError = (erroredId) => {
    setErroredList(erroredList.filter((id) => ![erroredId].includes(id)));
  };
  const removeTracks = useCallback(
    (selectedIds) => {
      try {
        nextSelectedIndexRef.current = trackIds.indexOf(selectedIds[0]);
        backend.removeTracks(selectedIds);
        setTrackIds((trackIds) => trackIds.filter((id) => !selectedIds.includes(id)));
        setErroredList(erroredList.filter((id) => !selectedIds.includes(id)));

        setRefreshList(backend.applyTrackListChanges());

        waitingIdsRef.current = waitingIdsRef.current.concat(selectedIds);
        if (waitingIdsRef.current.length > 1) {
          waitingIdsRef.current.sort((a, b) => a - b);
        }
      } catch (err) {
        console.log(err);
        alert("Could not remove track");
      }
    },
    [trackIds, erroredList],
  );

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
