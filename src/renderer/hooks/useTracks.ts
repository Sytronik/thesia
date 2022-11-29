import {useRef, useState, useCallback} from "react";
import {ipcRenderer} from "electron";
import {SUPPORTED_TYPES} from "renderer/prototypes/constants";
import {difference} from "renderer/utils/arrayUtils";
import NativeAPI from "../api";

export default function useTracks() {
  const [trackIds, setTrackIds] = useState<number[]>([]);
  const [erroredList, setErroredList] = useState<number[]>([]);
  const [refreshList, setRefreshList] = useState<IdChArr>([]);

  const waitingIdsRef = useRef<number[]>([]);

  const reloadTracks = useCallback(async (selectedIds: number[]) => {
    try {
      const reloadedIds = NativeAPI.reloadTracks(selectedIds);
      const erroredIds = difference(selectedIds, reloadedIds);
      const needRefreshIds = await NativeAPI.applyTrackListChanges();

      if (erroredIds && erroredIds.length) {
        setErroredList(erroredIds);
      }
      if (needRefreshIds) {
        setRefreshList(needRefreshIds);
      }
    } catch (err) {
      console.log("Track reloads error", err);
    }
  }, []);

  const addTracks = useCallback(
    (newPaths: string[], unsupportedPaths: string[]) => {
      try {
        const newIds: number[] = [];
        const existingIds: number[] = [];
        let invalidIds: number[] = [];
        let invalidPaths: string[] = [];

        newPaths.forEach((path, i, newPaths) => {
          const id = NativeAPI.findIdByPath(path);
          if (id !== -1) {
            newPaths.splice(i, 1);
            existingIds.push(id);
          }
        });

        if (newPaths.length) {
          for (let i = 0; i < newPaths.length; i += 1) {
            if (waitingIdsRef.current.length) {
              newIds.push(waitingIdsRef.current.shift() as number);
            } else {
              newIds.push(trackIds.length + i);
            }
          }

          // nextSelectedIndexRef.current = trackIds.length;
          const addedIds = NativeAPI.addTracks(newIds, newPaths);
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

        /*
        if (existingIds.length) {
          reloadTracks(existingIds);
        } else {
          setRefreshList(NativeAPI.applyTrackListChanges());
        }
        */
      } catch (err) {
        console.log(err);
        alert("File upload error");
      }
    },
    [trackIds],
  );

  const ignoreError = (erroredId: number) => {
    setErroredList(erroredList.filter((id) => ![erroredId].includes(id)));
  };

  const removeTracks = useCallback(
    (selectedIds) => {
      try {
        // nextSelectedIndexRef.current = trackIds.indexOf(selectedIds[0]);
        NativeAPI.removeTracks(selectedIds);
        setTrackIds((trackIds) => trackIds.filter((id) => !selectedIds.includes(id)));
        setErroredList(erroredList.filter((id) => !selectedIds.includes(id)));

        // setRefreshList(NativeAPI.applyTrackListChanges());

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

  return {
    trackIds,
    erroredList,
    refreshList,
    reloadTracks,
    addTracks,
    removeTracks,
    ignoreError,
  };
}
