import React, { useState } from "react";
import "./App.scss";
import Control from "./components/Control/Control"
import Overview from "./components/Overview/Overview"
import SlideBar from "./components/SlideBar/SlideBar"
import MainViewer from "./components/MainViewer/MainViewer"
import ColorBar from "./components/ColorBar/ColorBar"

import path from "path";

const p = window.preload;
const native = p.native;
const {dialog} = p.remote;
const __dirname = p.__dirname;

const supported_types = ['flac', 'mp3', 'oga', 'ogg', 'wav'];
const supported_MIME = supported_types.map(subtype => `audio/${subtype}`);

function App() {

  const [track_ids, setTrackIds] = useState([]);
  const [refresh_list, setRefreshList] = useState(null);

  async function addTracks(new_paths, unsupported_paths) {
    try {
      let invalid_ids = [];
      let invalid_paths = [];

      const new_track_ids = [...new_paths.keys()]; // [Temp]

      const [added_ids, promise_refresh_list] = native.addTracks(new_track_ids, new_paths);
      setTrackIds(track_ids => track_ids.concat(added_ids));
      setRefreshList(await promise_refresh_list);

      if (new_track_ids.length !== added_ids.length) {
        invalid_ids = new_track_ids.filter(id => !added_ids.includes(id));
        invalid_paths = invalid_ids.map(id => new_paths[(new_track_ids.indexOf(id))]);
      }
      if (unsupported_paths.length || invalid_paths.length) {
        dialog.showMessageBox({ 
          type: 'error', 
          buttons: [], 
          defaultId: 0, 
          icon: '', 
          title: 'File Open Error', 
          message: 'The following files could not be opened', 
          detail: `${unsupported_paths.length ? `-- Not Supported Type --
                                                ${unsupported_paths.join('\n')}
                                                ` : ''}\
                    ${invalid_paths.length ? `-- Not Valid Format --
                                            ${invalid_paths.join('\n')}
                                            ` : ''}\
                    
                    Please ensure that the file properties are correct and that it is a supported file type.
                    Only files with the following extensions are allowed: ${supported_types.join(', ')}`,
          cancelId: 0, 
          noLink: false, 
          normalizeAccessKeys: false, 
        });
      }
    } catch(err) {
      console.log(err);
      alert('File upload error');
    }
  }

  async function openDialog() {
    const file = await dialog.showOpenDialog({ 
      title: 'Select the File to be uploaded', 
      defaultPath: path.join(__dirname, '/samples/'), 
      filters: [ 
        { 
          name: 'Audio Files', 
          extensions: supported_types
        }, ], 
      properties: ['openFile', 'multiSelections']
    });

    if (!file.canceled) { 
      const new_paths = file.filePaths;
      const unsupported_paths = [];

      addTracks(new_paths, unsupported_paths);
    } else {
      console.log('file canceled: ', file.canceled);
    }
  }

  function dropFile(e) {
    e.preventDefault(); 
    e.stopPropagation(); 
    
    e.target.style.border = 'none';

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
          native={p.native} 
          dropFile={dropFile}
          openDialog={openDialog} 
          refresh_list={refresh_list} 
          track_ids={track_ids}
        />
        <ColorBar />
      </div>
    </div>
  );
}

export default App;
