import {useRef, useState, useMemo} from "react";
import {difference} from "renderer/utils/arrayUtils";
import useEvent from "react-use-event-hook";
import {setUserSetting} from "renderer/lib/ipc-sender";
import update from "immutability-helper";
import {UserSettings} from "backend";
import BackendAPI, {SpecSetting, GuardClippingMode, NormalizeTarget} from "../api";

type AddTracksResultType = {
  existingIds: number[];
  invalidPaths: string[];
};

function useTracks(userSettings: UserSettings) {
  const [trackIds, setTrackIds] = useState<number[]>([]);
  const [erroredTrackIds, setErroredTrackIds] = useState<number[]>([]);
  const [needRefreshTrackIdChArr, setNeedRefreshTrackIdChArr] = useState<IdChArr>([]);

  const [currentSpecSetting, setCurrentSpecSetting] = useState<SpecSetting>(
    userSettings.specSetting,
  );
  const [blend, setBlend] = useState<number>(userSettings.blend);
  const [currentdBRange, setCurrentdBRange] = useState<number>(userSettings.dBRange);
  const [currentCommonGuardClipping, setCurrentCommonGuardClipping] = useState<GuardClippingMode>(
    userSettings.commonGuardClipping,
  );
  const [currentCommonNormalize, setCurrentCommonNormalize] = useState<NormalizeTarget>(
    userSettings.commonNormalize,
  );

  const waitingIdsRef = useRef<Set<number>>(new Set());
  const addToWaitingIds = useEvent((ids: number[]) => {
    ids.forEach((id) => waitingIdsRef.current.add(id));
  });

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const maxTrackSec = useMemo(BackendAPI.getLongestTrackLengthSec, [trackIds]);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const maxTrackHz = useMemo(BackendAPI.getMaxTrackHz, [trackIds, needRefreshTrackIdChArr]);

  const trackIdChMap: IdChMap = useMemo(
    () =>
      new Map(
        trackIds.map((id) => [
          id,
          [...Array(BackendAPI.getChannelCounts(id)).keys()].map((ch) => `${id}_${ch}`),
        ]),
      ),
    [trackIds],
  );

  const reloadTracks = useEvent(async (ids: number[]) => {
    try {
      const reloadedIds = await BackendAPI.reloadTracks(ids);
      const erroredIds = difference(ids, reloadedIds);

      if (erroredIds && erroredIds.length) setErroredTrackIds(erroredIds);

      if (reloadedIds.length > 0) setTrackIds((prevTrackIds) => prevTrackIds.slice());
    } catch (err) {
      console.error("Could not reload tracks", err);
    }
  });

  const refreshTracks = useEvent(async () => {
    try {
      const needRefreshIdChArr = await BackendAPI.applyTrackListChanges();
      if (needRefreshIdChArr) {
        setNeedRefreshTrackIdChArr(needRefreshIdChArr);
      }
    } catch (err) {
      console.error("Could not refresh tracks", err);
    }
  });

  const addTracks = useEvent(async (paths: string[]): Promise<AddTracksResultType> => {
    try {
      const idsOfInputPaths = await Promise.all(paths.map(BackendAPI.findIdByPath));
      const newPaths = paths.filter((_, index) => idsOfInputPaths[index] === -1);
      const existingIds = idsOfInputPaths.filter((id) => id !== -1);

      if (!newPaths.length) return {existingIds, invalidPaths: []};

      const newIds = [...Array(newPaths.length).keys()].map((i) => {
        if (waitingIdsRef.current.size > 0) {
          const id = waitingIdsRef.current.values().next().value as number;
          waitingIdsRef.current.delete(id);
          return id;
        }
        return trackIds.length + i;
      });

      const addedIds = await BackendAPI.addTracks(newIds, newPaths);
      if (addedIds.length) {
        setTrackIds((prevTrackIds) => prevTrackIds.concat(addedIds));
      }

      if (newIds.length === addedIds.length) return {existingIds, invalidPaths: []};

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

  const removeTracks = useEvent((ids: number[]) => {
    try {
      BackendAPI.removeTracks(ids);
      setTrackIds((prevTrackIds) => difference(prevTrackIds, ids));
      setErroredTrackIds((prevErroredTrackIds) => difference(prevErroredTrackIds, ids));

      addToWaitingIds(ids);
    } catch (err) {
      console.error("Could not remove track", err);
      alert("Could not remove track");
    }
  });

  const changeTrackOrder = useEvent((dragIndex: number, hoverIndex: number) => {
    setTrackIds((prevTrackOrder) =>
      update(prevTrackOrder, {
        $splice: [
          [dragIndex, 1],
          [hoverIndex, 0, prevTrackOrder[dragIndex]],
        ],
      }),
    );
  });

  const setSpecSetting = useEvent(async (v: SpecSetting) => {
    await BackendAPI.setSpecSetting(v);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
    const specSetting = BackendAPI.getSpecSetting();
    setCurrentSpecSetting(specSetting);
    setUserSetting("specSetting", specSetting);
  });

  const setBlendAndSetUserSetting = useEvent((v: number) => {
    setBlend(v);
    setUserSetting("blend", v);
  });

  const setdBRange = useEvent(async (v: number) => {
    await BackendAPI.setdBRange(v);
    const dBRange = await BackendAPI.getdBRange();
    setCurrentdBRange(dBRange);
    setUserSetting("dBRange", dBRange);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
  });

  const setCommonGuardClipping = useEvent(async (v: GuardClippingMode) => {
    await BackendAPI.setCommonGuardClipping(v);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
    const commonGuardClipping = BackendAPI.getCommonGuardClipping();
    setCurrentCommonGuardClipping(commonGuardClipping);
    setUserSetting("commonGuardClipping", commonGuardClipping);
  });

  const setCommonNormalize = useEvent(async (v: NormalizeTarget) => {
    await BackendAPI.setCommonNormalize(v);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
    const commonNormalize = BackendAPI.getCommonNormalize();
    setCurrentCommonNormalize(commonNormalize);
    setUserSetting("commonNormalize", commonNormalize);
  });

  const finishRefreshTracks = useEvent(() => {
    setNeedRefreshTrackIdChArr([]);
  });

  return {
    trackIds,
    erroredTrackIds,
    trackIdChMap,
    needRefreshTrackIdChArr,
    maxTrackSec,
    maxTrackHz,
    specSetting: currentSpecSetting,
    blend,
    dBRange: currentdBRange,
    commonNormalize: currentCommonNormalize,
    commonGuardClipping: currentCommonGuardClipping,
    reloadTracks,
    refreshTracks,
    addTracks,
    removeTracks,
    changeTrackOrder,
    ignoreError,
    setSpecSetting,
    setBlend: setBlendAndSetUserSetting,
    setdBRange,
    setCommonNormalize,
    setCommonGuardClipping,
    finishRefreshTracks,
  };
}

export default useTracks;
