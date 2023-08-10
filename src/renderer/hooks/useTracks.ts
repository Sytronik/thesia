import {useRef, useState, useCallback} from "react";
import {difference} from "renderer/utils/arrayUtils";
import NativeAPI from "../api";

type AddTracksResultType = {
  existingIds: number[];
  invalidPaths: string[];
};

function useTracks() {
  const [trackIds, setTrackIds] = useState<number[]>([]);
  const [erroredTrackIds, setErroredTrackIds] = useState<number[]>([]);
  const [needRefreshTrackIds, setNeedRefreshTrackIds] = useState<IdChArr>([]);

  const waitingIdsRef = useRef<number[]>([]);
  const addToWaitingIds = useCallback((ids: number[]) => {
    waitingIdsRef.current = waitingIdsRef.current.concat(ids);
    if (waitingIdsRef.current.length > 1) {
      waitingIdsRef.current.sort((a, b) => a - b);
    }
  }, []);

  const reloadTracks = useCallback(async (ids: number[]) => {
    try {
      const reloadedIds = await NativeAPI.reloadTracks(ids);
      const erroredIds = difference(ids, reloadedIds);

      if (erroredIds && erroredIds.length) {
        setErroredTrackIds(erroredIds);
      }
    } catch (err) {
      console.error("Could not reload tracks", err);
    }
  }, []);

  const refreshTracks = useCallback(async () => {
    try {
      const needRefreshIds = await NativeAPI.applyTrackListChanges();
      if (needRefreshIds) {
        setNeedRefreshTrackIds(needRefreshIds);
      }
    } catch (err) {
      console.error("Could not refresh tracks", err);
    }
  }, []);

  const addTracks = useCallback(
    async (paths: string[]): Promise<AddTracksResultType> => {
      try {
        const newPaths = paths.filter(async (path) => (await NativeAPI.findIdByPath(path)) === -1);
        const existingPaths = difference(paths, newPaths);
        const existingIds = await Promise.all(
          existingPaths.map(async (path) => NativeAPI.findIdByPath(path)),
        );

        if (!newPaths.length) {
          return {existingIds, invalidPaths: []};
        }

        const newIds = [...Array(newPaths.length).keys()].map((i) => {
          if (waitingIdsRef.current.length) {
            return waitingIdsRef.current.shift() as number;
          }
          return trackIds.length + i;
        });

        const addedIds = await NativeAPI.addTracks(newIds, newPaths);
        if (addedIds.length) {
          setTrackIds((prevTrackIds) => prevTrackIds.concat(addedIds));
        }

        if (newIds.length === addedIds.length) {
          return {existingIds, invalidPaths: []};
        }

        const invalidIds = difference(newIds, addedIds);
        const invalidPaths = invalidIds.map((id) => newPaths[newIds.indexOf(id)]);
        addToWaitingIds(invalidIds);

        return {existingIds, invalidPaths};
      } catch (err) {
        console.error("Track adds error", err);
        alert("Track adds error");

        return {existingIds: [], invalidPaths: []};
      }
    },
    [trackIds, addToWaitingIds],
  );

  const ignoreError = useCallback((erroredId: number) => {
    setErroredTrackIds((prevErroredTrackIds) => difference(prevErroredTrackIds, [erroredId]));
  }, []);

  const removeTracks = useCallback(
    async (ids: number[]) => {
      try {
        await NativeAPI.removeTracks(ids);
        setTrackIds((prevTrackIds) => difference(prevTrackIds, ids));
        setErroredTrackIds((prevErroredTrackIds) => difference(prevErroredTrackIds, ids));

        addToWaitingIds(ids);
      } catch (err) {
        console.error("Could not remove track", err);
        alert("Could not remove track");
      }
    },
    [addToWaitingIds],
  );

  return {
    trackIds,
    erroredTrackIds,
    needRefreshTrackIds,
    reloadTracks,
    refreshTracks,
    addTracks,
    removeTracks,
    ignoreError,
  };
}

export default useTracks;
