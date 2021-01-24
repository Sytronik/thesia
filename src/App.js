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
const dialog = p.dialog;
const __dirname = p.__dirname;

function App() {

  const [track_ids, setTrackIds] = useState([]);
  const [refresh_list, setRefreshList] = useState(null);

  async function openDialog() {
    try {
      const file = await dialog.showOpenDialog({ 
        title: 'Select the File to be uploaded', 
        defaultPath: path.join(__dirname, '/samples/'), 
        filters: [ 
          { 
            name: 'Audio Files', 
            extensions: ['wav'] 
          }, ], 
        properties: ['openFile', 'multiSelections']
      })

      if (!file.canceled) { 
        const new_paths = file.filePaths;
        const new_track_ids = [...new_paths.keys()]; // [Temp]

        const [added_ids, promise_refresh_list] = native.addTracks(new_track_ids, new_paths);
        console.log(added_ids); // TODO: alert unsupported files to user
        setTrackIds(track_ids => track_ids.concat(added_ids));
        setRefreshList(await promise_refresh_list);
      } else {
        console.log('file canceled: ', file.canceled);
      }
    } catch(err) { 
      console.log(err);
      alert('file upload error');
    }; 
  };

  async function dropFile(e) {
    e.preventDefault(); 
    e.stopPropagation(); 
    
    try {
      e.target.style.border = 'none';

      const new_paths = [];
      for (const file of e.dataTransfer.files) {
        new_paths.push(file.path);
      };
      const new_track_ids = [...new_paths.keys()]; // [Temp]

      const [added_ids, promise_refresh_list] = native.addTracks(new_track_ids, new_paths);
      console.log(added_ids); // TODO: alert unsupported files to user
      setTrackIds(track_ids => track_ids.concat(added_ids));
      setRefreshList(await promise_refresh_list);
    } catch(err) {
      console.log(err);
      alert('file upload error');
    };
  }; 

  return (
    <div className="App">
      <Control />
      <div>
        <Overview />
        <SlideBar />
      </div>
      <div>
        <ColorBar />
        <MainViewer
          native={p.native} 
          dropFile={dropFile}
          openDialog={openDialog} 
          refresh_list={refresh_list} 
          track_ids={track_ids}
        />
      </div>
    </div>
  );
}

export default App;
