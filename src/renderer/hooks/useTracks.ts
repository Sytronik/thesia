import {useRef, useState, useMemo} from "react";
import {difference} from "renderer/utils/arrayUtils";
import useEvent from "react-use-event-hook";
import NativeAPI from "../api";

type AddTracksResultType = {
  existingIds: number[];
  invalidPaths: string[];
};

function useTracks() {
  const [trackIds, setTrackIds] = useState<number[]>([]);
  const [erroredTrackIds, setErroredTrackIds] = useState<number[]>([]);
  const [needRefreshTrackIdChArr, setNeedRefreshTrackIdChArr] = useState<IdChArr>([]);
  const [currentSpecSetting, setCurrentSpecSetting] = useState<SpecSetting>(
    NativeAPI.getSpecSetting(),
  );

  const waitingIdsRef = useRef<number[]>([]);
  const addToWaitingIds = useEvent((ids: number[]) => {
    waitingIdsRef.current = waitingIdsRef.current.concat(ids);
    if (waitingIdsRef.current.length > 1) {
      waitingIdsRef.current.sort((a, b) => a - b);
    }
  });

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const maxTrackSec = useMemo(() => NativeAPI.getLongestTrackLength(), [trackIds]);
  const trackIdChMap: IdChMap = useMemo(
    () =>
      new Map(
        trackIds.map((id) => [
          id,
          [...Array(NativeAPI.getChannelCounts(id)).keys()].map((ch) => `${id}_${ch}`),
        ]),
      ),
    [trackIds],
  );

  const reloadTracks = useEvent(async (ids: number[]) => {
    try {
      const reloadedIds = await NativeAPI.reloadTracks(ids);
      const erroredIds = difference(ids, reloadedIds);

      if (erroredIds && erroredIds.length) {
        setErroredTrackIds(erroredIds);
      }
    } catch (err) {
      console.error("Could not reload tracks", err);
    }
  });

  const refreshTracks = useEvent(async () => {
    try {
      const needRefreshIdChArr = await NativeAPI.applyTrackListChanges();
      if (needRefreshIdChArr) {
        setNeedRefreshTrackIdChArr(needRefreshIdChArr);
      }
    } catch (err) {
      console.error("Could not refresh tracks", err);
    }
  });

  const addTracks = useEvent(async (paths: string[]): Promise<AddTracksResultType> => {
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
  });

  const ignoreError = useEvent((erroredId: number) => {
    setErroredTrackIds((prevErroredTrackIds) => difference(prevErroredTrackIds, [erroredId]));
  });

  const removeTracks = useEvent(async (ids: number[]) => {
    try {
      await NativeAPI.removeTracks(ids);
      setTrackIds((prevTrackIds) => difference(prevTrackIds, ids));
      setErroredTrackIds((prevErroredTrackIds) => difference(prevErroredTrackIds, ids));

      addToWaitingIds(ids);
    } catch (err) {
      console.error("Could not remove track", err);
      alert("Could not remove track");
    }
  });

  const setSpecSetting = useEvent(async (specSetting: SpecSetting) => {
    await NativeAPI.setSpecSetting(specSetting);
    setCurrentSpecSetting(NativeAPI.getSpecSetting());
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
  });

  return {
    trackIds,
    erroredTrackIds,
    trackIdChMap,
    needRefreshTrackIdChArr,
    maxTrackSec,
    specSetting: currentSpecSetting,
    reloadTracks,
    refreshTracks,
    addTracks,
    removeTracks,
    ignoreError,
    setSpecSetting,
  };
}

export default useTracks;
