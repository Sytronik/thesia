import React, {useEffect, useRef, useState} from "react";
import "./App.scss";
import Control from "./components/Control/Control";
import Overview from "./components/Overview/Overview";
import SlideBar from "./components/SlideBar/SlideBar";
import MainViewer from "./components/MainViewer/MainViewer";
import ColorBar from "./components/ColorBar/ColorBar";

import path from "path";

const {__dirname, remote, native} = window.preload;
const {dialog, Menu, MenuItem} = remote;

const supported_types = ["flac", "mp3", "oga", "ogg", "wav"];
const supported_MIME = supported_types.map((subtype) => `audio/${subtype}`);

function App() {
  const temp_ids = useRef([]);
  const selected_list = useRef([]);
  const selected_next = useRef(null);

  const [track_ids, setTrackIds] = useState([]);
  const [refresh_list, setRefreshList] = useState(null);

  async function addTracks(new_paths, unsupported_paths) {
    try {
      let invalid_ids = [];
      let invalid_paths = [];
      let new_track_ids = [];

      if (!track_ids.length) {
        new_track_ids = [...new_paths.keys()];
      } else {
        for (let i = 0; i < new_paths.length; i++) {
          if (temp_ids.current.length) {
            new_track_ids.push(temp_ids.current.shift());
          } else {
            new_track_ids.push(track_ids.length + i);
          }
        }
      }

      selected_next.current = track_ids.length;
      const [added_ids, promise_refresh_list] = native.addTracks(new_track_ids, new_paths);
      setTrackIds((track_ids) => track_ids.concat(added_ids));
      setRefreshList(await promise_refresh_list);

      if (new_track_ids.length !== added_ids.length) {
        invalid_ids = new_track_ids.filter((id) => !added_ids.includes(id));
        invalid_paths = invalid_ids.map((id) => new_paths[new_track_ids.indexOf(id)]);
      }
      if (unsupported_paths.length || invalid_paths.length) {
        dialog.showMessageBox({
          type: "error",
          buttons: [],
          defaultId: 0,
          icon: "",
          title: "File Open Error",
          message: "The following files could not be opened",
          detail: `${
            unsupported_paths.length
              ? `-- Not Supported Type --
              ${unsupported_paths.join("\n")}
              `
              : ""
          }\
          ${
            invalid_paths.length
              ? `-- Not Valid Format --
              ${invalid_paths.join("\n")}
              `
              : ""
          }\
          
          Please ensure that the file properties are correct and that it is a supported file type.
          Only files with the following extensions are allowed: ${supported_types.join(", ")}`,
          cancelId: 0,
          noLink: false,
          normalizeAccessKeys: false,
        });
      }
    } catch (err) {
      console.log(err);
      alert("File upload error");
    }
  }
  async function removeTracks(ids) {
    try {
      selected_next.current = track_ids.indexOf(ids[0]);
      const promise_refresh_list = native.removeTracks(ids);
      setTrackIds((track_ids) => track_ids.filter((id) => !ids.includes(id)));

      if (promise_refresh_list) {
        setRefreshList(await promise_refresh_list);
      }

      temp_ids.current = temp_ids.current.concat(ids);
      if (temp_ids.current.length > 1) {
        temp_ids.current.sort((a, b) => a - b);
      }
    } catch (err) {
      console.log(err);
      alert("Could not remove track");
    }
  }

  async function openDialog() {
    const file = await dialog.showOpenDialog({
      title: "Select the File to be uploaded",
      defaultPath: path.join(__dirname, "/samples/"),
      filters: [
        {
          name: "Audio Files",
          extensions: supported_types,
        },
      ],
      properties: ["openFile", "multiSelections"],
    });

    if (!file.canceled) {
      const new_paths = file.filePaths;
      const unsupported_paths = [];

      addTracks(new_paths, unsupported_paths);
    } else {
      console.log("file canceled: ", file.canceled);
    }
  }
  function dropFile(e) {
    e.preventDefault();
    e.stopPropagation();

    const new_paths = [];
    const unsupported_paths = [];

    for (const file of e.dataTransfer.files) {
      if (supported_MIME.includes(file.type)) {
        new_paths.push(file.path);
      } else {
        unsupported_paths.push(file.path);
      }
    }
    addTracks(new_paths, unsupported_paths);
  }

  const setSelected = (selected_list) => {
    const track_infos = document.querySelectorAll(".TrackInfo");
    track_infos.forEach((track_info) => {
      if (selected_list.includes(Number(track_info.getAttribute("trackid")))) {
        track_info.classList.add("selected");
      } else {
        track_info.classList.remove("selected");
      }
    });
  };
  const selectTrack = (e) => {
    e.preventDefault();

    const classlist = e.currentTarget.classList;
    const id = Number(e.currentTarget.getAttribute("trackid"));

    if (!classlist.contains("selected")) {
      selected_list.current = [id];
      setSelected(selected_list.current);
    }
  };
  const deleteSelected = (e) => {
    e.preventDefault();

    if (e.key === "Delete" || e.key === "Backspace") {
      if (selected_list.current.length) {
        removeTracks(selected_list.current);
      }
    }
  };
  const showContextMenu = (e) => {
    e.preventDefault();

    const id = Number(e.target.getAttribute("trackid"));
    const ids = [id];
    const menu = new Menu();
    menu.append(
      new MenuItem({
        label: "Delete Track",
        click() {
          removeTracks(ids);
        },
      }),
    );

    menu.popup(remote.getCurrentWindow());
  };

  useEffect(() => {
    document.addEventListener("keydown", deleteSelected);

    return () => {
      document.removeEventListener("keydown", deleteSelected);
    };
  });

  useEffect(() => {
    const track_num = track_ids.length;
    if (!track_num) {
      selected_list.current = [];
    } else if (selected_next.current < track_num) {
      selected_list.current = [track_ids[selected_next.current]];
    } else {
      selected_list.current = [track_ids[track_num - 1]];
    }
    setSelected(selected_list.current);
    selected_next.current = null;
  }, [track_ids]);

  return (
    <div className="App">
      <div className="row-control">
        <Control />
      </div>
      <div className="row-overview">
        <Overview />
        <SlideBar />
      </div>
      <div className="row-mainviewer">
        <MainViewer
          refresh_list={refresh_list}
          track_ids={track_ids}
          dropFile={dropFile}
          openDialog={openDialog}
          selectTrack={selectTrack}
          showContextMenu={showContextMenu}
        />
        <ColorBar />
      </div>
    </div>
  );
}

export default App;
