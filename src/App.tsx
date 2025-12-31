import {MemoryRouter as Router, Routes, Route} from "react-router-dom";
import {useEffect, useMemo, useRef} from "react";
import useEvent from "react-use-event-hook";
import {DndProvider} from "react-dnd";
import {TouchBackend} from "react-dnd-touch-backend";
import BackendAPI, {
  UserSettings,
  listenMenuOpenAudioTracks,
  listenMenuRemoveSelectedTracks,
  listenOpenFiles,
} from "./api";
import {useHotkeys} from "react-hotkeys-hook";
import Control from "./prototypes/Control/Control";
import MainViewer from "./prototypes/MainViewer/MainViewer";
import PlayerControl from "./prototypes/PlayerControl/PlayerControl";
import {SUPPORTED_TYPES} from "./prototypes/constants/tracks";
import useTracks from "./hooks/useTracks";
import useSelectedTracks from "./hooks/useSelectedTracks";
import {DevicePixelRatioProvider} from "./contexts";
import usePlayer from "./hooks/usePlayer";
import "./App.scss";
import {useGlobalEvents} from "./hooks/useGlobalEvents";
import {showFileOpenErrorMsg, getOpenFilesDialogHandler} from "./lib/open-files-dialog";
import {initializeWorkerPool} from "./lib/worker-pool";
import {useOverlayScrollbars} from "./hooks/useOverlayScrollbars";

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

  useGlobalEvents();

  const player = usePlayer(
    selectedTrackIds.length > 0 &&
      !erroredTrackIds.includes(selectedTrackIds[selectedTrackIds.length - 1])
      ? selectedTrackIds[selectedTrackIds.length - 1]
      : -1,
    maxTrackSec,
  );

  const prevTrackIds = useRef<number[]>([]); // includes hidden tracks

  const addDroppedFile = useEvent(async (paths: string[], index: number) => {
    if (paths.length === 0) {
      console.error("no file dropped");
      return;
    }

    const newPaths: string[] = [];
    const unsupportedPaths: string[] = [];

    paths.forEach((path) => {
      const extension = path.split(".").pop();
      if (extension && SUPPORTED_TYPES.includes(extension)) newPaths.push(path);
      else unsupportedPaths.push(path);
    });

    const {existingIds, invalidPaths} = await addTracks(newPaths, index);

    if (unsupportedPaths.length || invalidPaths.length) {
      showFileOpenErrorMsg(unsupportedPaths, invalidPaths);
    }
    if (existingIds.length) {
      await reloadTracks(existingIds);
    }
    await refreshTracks();
  });

  const openAudioTracks = useEvent(async (filePaths: string[]) => {
    const unsupportedPaths: string[] = [];

    const {existingIds, invalidPaths} = await addTracks(filePaths);

    if (unsupportedPaths.length || invalidPaths.length) {
      showFileOpenErrorMsg(unsupportedPaths, invalidPaths);
    }

    if (existingIds.length) {
      await reloadTracks(existingIds);
    }
    await refreshTracks();
  });

  useEffect(() => {
    const openedFiles: string[] | undefined = (window as any).openedFiles;
    if (openedFiles) {
      openAudioTracks(openedFiles);
    }
    delete (window as any).openedFiles;
    const promiseUnlisten = listenOpenFiles(openAudioTracks);
    return () => {
      promiseUnlisten.then((unlistenFn) => unlistenFn());
    };
  }, [openAudioTracks]);

  const openAudioTracksHandler = useMemo(
    () => getOpenFilesDialogHandler(openAudioTracks),
    [openAudioTracks],
  );

  useEffect(() => {
    const promiseUnlisten = listenMenuOpenAudioTracks(openAudioTracksHandler);
    return () => {
      promiseUnlisten.then((unlistenFn) => unlistenFn());
    };
  }, [openAudioTracksHandler]);

  const removeSelectedTracks = useEvent(async () => {
    if (selectedTrackIds.length === 0) return;
    removeTracks(selectedTrackIds);
    await refreshTracks();
  });

  useHotkeys("backspace, delete", removeSelectedTracks, {preventDefault: true}, [
    removeSelectedTracks,
  ]);
  useEffect(() => {
    const promiseUnlisten = listenMenuRemoveSelectedTracks(removeSelectedTracks);
    return () => {
      promiseUnlisten.then((unlistenFn) => unlistenFn());
    };
  }, [removeSelectedTracks]);

  useEffect(() => {
    const prevTrackIdsCount = prevTrackIds.current.length;
    const currTrackIdsCount = trackIds.length + hiddenTrackIds.length;

    if (prevTrackIdsCount === currTrackIdsCount) return;

    BackendAPI.changeMenuDepsOnTrackExistence(currTrackIdsCount > 0);

    if (prevTrackIdsCount < currTrackIdsCount) {
      selectTrackAfterAddTracks(prevTrackIds.current, trackIds);
    } else {
      selectTrackAfterRemoveTracks(prevTrackIds.current, trackIds);
    }

    if (currTrackIdsCount == 0) initializeWorkerPool();

    prevTrackIds.current = trackIds.concat(hiddenTrackIds);
  }, [trackIds, hiddenTrackIds, selectTrackAfterAddTracks, selectTrackAfterRemoveTracks]);

  useOverlayScrollbars();

  return (
    <div id="App" className="App">
      <DndProvider backend={TouchBackend} options={{enableMouseEvents: true}}>
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
              openAudioTracksHandler={openAudioTracksHandler}
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
  return (
    <Router>
      <Routes>
        <Route path="/" element={<MyApp userSettings={userSettings} />} />
      </Routes>
    </Router>
  );
}
