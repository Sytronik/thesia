import {useRef, useState, useMemo} from "react";
import {difference} from "renderer/utils/arrayUtils";
import useEvent from "react-use-event-hook";
import {setUserSetting} from "renderer/lib/ipc-sender";
import BackendAPI, {SpecSetting, GuardClippingMode, NormalizeTarget} from "../api";

type AddTracksResultType = {
  existingIds: number[];
  invalidPaths: string[];
};

function getInitialValue<K extends keyof UserSettings>(
  userSettings: UserSettings,
  key: K,
  getFromBackend: () => NonNullable<UserSettings[K]>,
  setToBackend: (v: NonNullable<UserSettings[K]>) => void,
): NonNullable<UserSettings[K]> {
  const value = userSettings[key];
  if (value !== undefined) {
    setToBackend(value);
    return value;
  }
  const valueFromBackend = getFromBackend();
  setUserSetting(key, valueFromBackend);
  return valueFromBackend;
}

function useTracks(userSettings: UserSettings) {
  const [trackIds, setTrackIds] = useState<number[]>([]);
  const [erroredTrackIds, setErroredTrackIds] = useState<number[]>([]);
  const [needRefreshTrackIdChArr, setNeedRefreshTrackIdChArr] = useState<IdChArr>([]);

  const [currentSpecSetting, setCurrentSpecSetting] = useState<SpecSetting>(() =>
    getInitialValue(
      userSettings,
      "specSetting",
      BackendAPI.getSpecSetting,
      BackendAPI.setSpecSetting,
    ),
  );

  const [blend, setBlend] = useState<number>(() => {
    if (userSettings.blend !== undefined) return userSettings.blend;
    setUserSetting("blend", 0.5);
    return 0.5;
  });

  const [currentdBRange, setCurrentdBRange] = useState<number>(() =>
    getInitialValue(userSettings, "dBRange", BackendAPI.getdBRange, BackendAPI.setdBRange),
  );

  const [currentCommonGuardClipping, setCurrentCommonGuardClipping] = useState<GuardClippingMode>(
    () =>
      getInitialValue(
        userSettings,
        "commonGuardClipping",
        BackendAPI.getCommonGuardClipping,
        BackendAPI.setCommonGuardClipping,
      ),
  );

  const [currentCommonNormalize, setCurrentCommonNormalize] = useState<NormalizeTarget>(() =>
    getInitialValue(
      userSettings,
      "commonNormalize",
      BackendAPI.getCommonNormalize,
      BackendAPI.setCommonNormalize,
    ),
  );

  const waitingIdsRef = useRef<number[]>([]);
  const addToWaitingIds = useEvent((ids: number[]) => {
    waitingIdsRef.current = waitingIdsRef.current.concat(ids);
    if (waitingIdsRef.current.length > 1) {
      waitingIdsRef.current.sort((a, b) => a - b);
    }
  });

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const maxTrackSec = useMemo(() => BackendAPI.getLongestTrackLengthSec(), [trackIds]);
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

      if (erroredIds && erroredIds.length) {
        setErroredTrackIds(erroredIds);
      }
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
      const idsOfInputPaths = paths.map((path) => BackendAPI.findIdByPath(path));
      const newPaths = paths.filter((_, index) => idsOfInputPaths[index] === -1);
      const existingIds = idsOfInputPaths.filter((id) => id !== -1);

      if (!newPaths.length) return {existingIds, invalidPaths: []};

      const newIds = [...Array(newPaths.length).keys()].map((i) => {
        if (waitingIdsRef.current.length) {
          return waitingIdsRef.current.shift() as number;
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

  const setSpecSetting = useEvent(async (v: SpecSetting) => {
    await BackendAPI.setSpecSetting(v);
    const specSetting = BackendAPI.getSpecSetting();
    setCurrentSpecSetting(specSetting);
    setUserSetting("specSetting", specSetting);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
  });

  const setBlendAndSetUserSetting = useEvent((v: number) => {
    setBlend(v);
    setUserSetting("blend", v);
  });

  const setdBRange = useEvent(async (v: number) => {
    BackendAPI.setdBRange(v);
    const dBRange = BackendAPI.getdBRange();
    setCurrentdBRange(dBRange);
    setUserSetting("dBRange", dBRange);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
  });

  const setCommonGuardClipping = useEvent(async (v: GuardClippingMode) => {
    await BackendAPI.setCommonGuardClipping(v);
    const commonGuardClipping = BackendAPI.getCommonGuardClipping();
    setCurrentCommonGuardClipping(commonGuardClipping);
    setUserSetting("commonGuardClipping", commonGuardClipping);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
  });

  const setCommonNormalize = useEvent(async (v: NormalizeTarget) => {
    await BackendAPI.setCommonNormalize(v);
    const commonNormalize = BackendAPI.getCommonNormalize();
    setCurrentCommonNormalize(commonNormalize);
    setUserSetting("commonNormalize", commonNormalize);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
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
    specSetting: currentSpecSetting,
    blend,
    dBRange: currentdBRange,
    commonNormalize: currentCommonNormalize,
    commonGuardClipping: currentCommonGuardClipping,
    reloadTracks,
    refreshTracks,
    addTracks,
    removeTracks,
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
