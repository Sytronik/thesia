import {useRef, useState, useCallback} from "react";
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

  const reloadTracks = useCallback(async (selectedIds: number[]) => {
    try {
      const reloadedIds = NativeAPI.reloadTracks(selectedIds);
      const erroredIds = selectedIds.filter((id) => !reloadedIds.includes(id));

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
        const existingPaths = paths.filter((path) => !newPaths.includes(path));
        const existingIds = existingPaths.map((path) => NativeAPI.findIdByPath(path));

        if (!newPaths.length) {
          return {existingIds, invalidPaths: []};
        }

        const newIds = [...Array(newPaths.length).keys()].map((i) => {
          if (waitingIdsRef.current.length) {
            return waitingIdsRef.current.shift() as number;
          }
          return trackIds.length + i;
        });

        // nextSelectedIndexRef.current = trackIds.length;
        const addedIds = NativeAPI.addTracks(newIds, newPaths);
        setTrackIds((prevTrackIds) => prevTrackIds.concat(addedIds));

        waitingIdsRef.current = waitingIdsRef.current.slice(newPaths.length);

        if (newIds.length === addedIds.length) {
          return {existingIds, invalidPaths: []};
        }

        const invalidIds = newIds.filter((path) => !addedIds.includes(path));
        const invalidPaths = invalidIds.map((id) => newPaths[newIds.indexOf(id)]);

        waitingIdsRef.current = waitingIdsRef.current.concat(invalidIds);
        if (waitingIdsRef.current.length > 1) {
          waitingIdsRef.current.sort((a, b) => a - b);
        }

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
    setErroredList((prevErroredList) => prevErroredList.filter((id) => ![erroredId].includes(id)));
  }, []);

  const removeTracks = useCallback(async (selectedIds) => {
    try {
      // nextSelectedIndexRef.current = trackIds.indexOf(selectedIds[0]);
      NativeAPI.removeTracks(selectedIds);
      setTrackIds((prevTrackIds) => prevTrackIds.filter((id) => !selectedIds.includes(id)));
      setErroredList((prevErroredList) =>
        prevErroredList.filter((id) => !selectedIds.includes(id)),
      );

      waitingIdsRef.current = waitingIdsRef.current.concat(selectedIds);
      if (waitingIdsRef.current.length > 1) {
        waitingIdsRef.current.sort((a, b) => a - b);
      }
    } catch (err) {
      console.log(err);
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
