import React, {useEffect, useRef, useState} from "react";
import "./App.scss";
import Control from "./components/Control/Control";
import Overview from "./components/Overview/Overview";
import SlideBar from "./components/SlideBar/SlideBar";
import MainViewer from "./components/MainViewer/MainViewer";
import {PROPERTY} from "./components/Property";

import path from "path";

const {__dirname, remote, native} = window.preload;
const {dialog, Menu, MenuItem} = remote;

const SUPPORTED_TYPES = PROPERTY.SUPPORTED_TYPES;
const SUPPORTED_MIME = SUPPORTED_TYPES.map((subtype) => `audio/${subtype}`);

function App() {
  const waitingIdsRef = useRef([]);
  const selectedIdsRef = useRef([]);
  const nextSelectedIndexRef = useRef(null);

  const [trackIds, setTrackIds] = useState([]);
  const [erroredList, setErroredList] = useState([]);
  const [refreshList, setRefreshList] = useState([]);

  async function reloadTracks(selectedIds) {
    const reloadedIds = native.reloadTracks(selectedIds);

    setErroredList(selectedIds.filter((id) => !reloadedIds.includes(id)));
    setRefreshList(native.applyTrackListChanges());
  }
  async function addTracks(newPaths, unsupportedPaths) {
    try {
      let newIds = [];
      let existingIds = [];
      let invalidIds = [];
      let invalidPaths = [];

      newPaths.forEach((path, i, newPaths) => {
        const id = native.findIDbyPath(path);
        if (id != -1) {
          newPaths.splice(i, 1);
          existingIds.push(id);
        }
      });

      if (newPaths.length) {
        for (let i = 0; i < newPaths.length; i++) {
          if (waitingIdsRef.current.length) {
            newIds.push(waitingIdsRef.current.shift());
          } else {
            newIds.push(trackIds.length + i);
          }
        }

        nextSelectedIndexRef.current = trackIds.length;
        const addedIds = native.addTracks(newIds, newPaths);
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
          dialog.showMessageBox({
            type: "error",
            buttons: [],
            defaultId: 0,
            icon: "",
            title: "File Open Error",
            message: "The following files could not be opened",
            detail: `${
              unsupportedPaths.length
                ? `-- Not Supported Type --
                ${unsupportedPaths.join("\n")}
                `
                : ""
            }\
            ${
              invalidPaths.length
                ? `-- Not Valid Format --
                ${invalidPaths.join("\n")}
                `
                : ""
            }\
            
            Please ensure that the file properties are correct and that it is a supported file type.
            Only files with the following extensions are allowed: ${SUPPORTED_TYPES.join(", ")}`,
            cancelId: 0,
            noLink: false,
            normalizeAccessKeys: false,
          });
        }
      }

      if (existingIds.length) {
        reloadTracks(existingIds);
      } else {
        setRefreshList(native.applyTrackListChanges());
      }
    } catch (err) {
      console.log(err);
      alert("File upload error");
    }
  }
  const ignoreError = (erroredId) => {
    setErroredList(erroredList.filter((id) => ![erroredId].includes(id)));
  };
  async function removeTracks(selectedIds) {
    try {
      nextSelectedIndexRef.current = trackIds.indexOf(selectedIds[0]);
      native.removeTracks(selectedIds);
      setTrackIds((trackIds) => trackIds.filter((id) => !selectedIds.includes(id)));
      setErroredList(erroredList.filter((id) => !selectedIds.includes(id)));

      setRefreshList(native.applyTrackListChanges());

      waitingIdsRef.current = waitingIdsRef.current.concat(selectedIds);
      if (waitingIdsRef.current.length > 1) {
        waitingIdsRef.current.sort((a, b) => a - b);
      }
    } catch (err) {
      console.log(err);
      alert("Could not remove track");
    }
  }

  async function showOpenDialog() {
    const file = await dialog.showOpenDialog({
      title: "Select the File to be uploaded",
      defaultPath: path.join(__dirname, "/samples/"),
      filters: [
        {
          name: "Audio Files",
          extensions: SUPPORTED_TYPES,
        },
      ],
      properties: ["openFile", "multiSelections"],
    });

    if (!file.canceled) {
      const newPaths = file.filePaths;
      const unsupportedPaths = [];

      addTracks(newPaths, unsupportedPaths);
    } else {
      console.log("file canceled: ", file.canceled);
    }
  }
  function addDroppedFile(e) {
    e.preventDefault();
    e.stopPropagation();

    const newPaths = [];
    const unsupportedPaths = [];

    for (const file of e.dataTransfer.files) {
      if (SUPPORTED_MIME.includes(file.type)) {
        newPaths.push(file.path);
      } else {
        unsupportedPaths.push(file.path);
      }
    }
    addTracks(newPaths, unsupportedPaths);
  }

  const assignSelectedClass = (selectedIds) => {
    const targets = document.querySelectorAll(".js-track-left");
    targets.forEach((target) => {
      if (selectedIds.includes(Number(target.getAttribute("id")))) {
        target.classList.add("selected");
      } else {
        target.classList.remove("selected");
      }
    });
  };
  const selectTrack = (e) => {
    e.preventDefault();

    const targetClassList = e.currentTarget.classList;
    const targetTrackId = Number(e.currentTarget.getAttribute("id"));

    if (!targetClassList.contains("selected")) {
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
  const showContextMenu = (e) => {
    e.preventDefault();

    const targetTrackId = Number(e.currentTarget.getAttribute("id"));
    const menu = new Menu();
    menu.append(
      new MenuItem({
        label: "Delete Track",
        click() {
          removeTracks([targetTrackId]);
        },
      }),
    );

    menu.popup(remote.getCurrentWindow());
  };

  useEffect(() => {
    document.addEventListener("keydown", deleteSelectedTracks);

    return () => {
      document.removeEventListener("keydown", deleteSelectedTracks);
    };
  });

  useEffect(() => {
    const track_count = trackIds.length;
    if (!track_count) {
      selectedIdsRef.current = [];
    } else if (nextSelectedIndexRef.current < track_count) {
      selectedIdsRef.current = [trackIds[nextSelectedIndexRef.current]];
    } else {
      selectedIdsRef.current = [trackIds[track_count - 1]];
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
        showContextMenu={showContextMenu}
      />
    </div>
  );
}

export default App;
