import {MemoryRouter as Router, Routes, Route} from "react-router-dom";
import React, {useEffect, useRef} from "react";
import useEvent from "react-use-event-hook";
import {ipcRenderer} from "electron";
import Control from "./prototypes/Control/Control";
import MainViewer from "./prototypes/MainViewer/MainViewer";
import PlayerControl from "./prototypes/PlayerControl/PlayerControl";
import {showElectronFileOpenErrorMsg} from "./lib/electron-sender";
import {SUPPORTED_MIME} from "./prototypes/constants/tracks";
import "./App.scss";
import useTracks from "./hooks/useTracks";
import useSelectedTracks from "./hooks/useSelectedTracks";
import {DevicePixelRatioProvider} from "./contexts";
import usePlayer from "./hooks/usePlayer";

function MyApp() {
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
  } = useTracks();
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
          await reloadTracks(existingIds);
        }
        await refreshTracks();
      } else {
        console.log("file canceled: ", file.canceled);
      }
    });

    return () => {
      ipcRenderer.removeAllListeners("open-dialog-closed");
    };
  }, [addTracks, reloadTracks, refreshTracks]);

  const removeSelectedTracks = useEvent(async (_, targetTrackId) => {
    if (selectedTrackIds.includes(targetTrackId)) removeTracks(selectedTrackIds);
    else removeTracks([targetTrackId]);
    await refreshTracks();
  });

  useEffect(() => {
    ipcRenderer.on("delete-track", removeSelectedTracks);
    return () => {
      ipcRenderer.removeAllListeners("delete-track");
    };
  }, [removeSelectedTracks]);

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

export default function App() {
  return (
    <Router>
      <Routes>
        <Route path="/" element={<MyApp />} />
      </Routes>
    </Router>
  );
}
