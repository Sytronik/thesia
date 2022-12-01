import {useRef, useState, useCallback} from "react";
import {difference} from "renderer/utils/arrayUtils";
import NativeAPI from "../api";

type AddTracksResultType = {
  existingIds: number[];
  invalidPaths: string[];
};

export default function useTracks() {
  const [trackIds, setTrackIds] = useState<number[]>([]);
  const [erroredList, setErroredList] = useState<number[]>([]);
  const [refreshList, setRefreshList] = useState<IdChArr>([]);

  const waitingIdsRef = useRef<number[]>([]);
  const addToWaitingIds = useCallback((ids: number[]) => {
    waitingIdsRef.current = waitingIdsRef.current.concat(ids);
    if (waitingIdsRef.current.length > 1) {
      waitingIdsRef.current.sort((a, b) => a - b);
    }
  }, []);

  const reloadTracks = useCallback(async (selectedIds: number[]) => {
    try {
      const reloadedIds = NativeAPI.reloadTracks(selectedIds);
      const erroredIds = difference(selectedIds, reloadedIds);

      if (erroredIds && erroredIds.length) {
        setErroredList(erroredIds);
      }
    } catch (err) {
      console.log("Track reloads error", err);
    }
  }, []);

  const refreshTracks = useCallback(async () => {
    try {
      const needRefreshIds = await NativeAPI.applyTrackListChanges();
      if (needRefreshIds) {
        setRefreshList(needRefreshIds);
      }
    } catch (err) {
      console.log("Track refresh error", err);
    }
  }, []);

  const addTracks = useCallback(
    (paths: string[]): AddTracksResultType => {
      try {
        const newPaths = paths.filter((path) => NativeAPI.findIdByPath(path) === -1);
        const existingPaths = difference(paths, newPaths);
        const existingIds = existingPaths.map((path) => NativeAPI.findIdByPath(path));

        if (!newPaths.length) {
          return {existingIds, invalidPaths: []};
        }

        const createNeededIdCount = Math.max(newPaths.length - waitingIdsRef.current.length, 0);
        const createdIds = [...Array(createNeededIdCount).keys()].map((i) => i + trackIds.length);
        const newIds = [...waitingIdsRef.current, ...createdIds];

        // nextSelectedIndexRef.current = trackIds.length;
        const addedIds = NativeAPI.addTracks(newIds, newPaths);
        setTrackIds((prevTrackIds) => prevTrackIds.concat(addedIds));

        waitingIdsRef.current = waitingIdsRef.current.slice(newPaths.length);

        if (newIds.length === addedIds.length) {
          return {existingIds, invalidPaths: []};
        }

        const invalidIds = difference(newIds, addedIds);
        const invalidPaths = invalidIds.map((id) => newPaths[newIds.indexOf(id)]);
        addToWaitingIds(invalidIds);

        return {existingIds, invalidPaths};
      } catch (err) {
        console.log("Track adds error", err);
        alert("Track adds error");

        return {existingIds: [], invalidPaths: []};
      }
    },
    [trackIds],
  );

  const ignoreError = useCallback((erroredId: number) => {
    setErroredList((prevErroredList) => difference(prevErroredList, [erroredId]));
  }, []);

  const removeTracks = useCallback((selectedIds: number[]) => {
    try {
      // nextSelectedIndexRef.current = trackIds.indexOf(selectedIds[0]);
      NativeAPI.removeTracks(selectedIds);
      setTrackIds((prevTrackIds) => difference(prevTrackIds, selectedIds));
      setErroredList((prevErroredList) => difference(prevErroredList, selectedIds));

      addToWaitingIds(selectedIds);
    } catch (err) {
      console.log("Could not remove track", err);
      alert("Could not remove track");
    }
  }, []);

  return {
    trackIds,
    erroredList,
    refreshList,
    reloadTracks,
    refreshTracks,
    addTracks,
    removeTracks,
    ignoreError,
  };
}
