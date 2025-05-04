import {useRef, useState, useMemo} from "react";
import {difference} from "renderer/utils/arrayUtils";
import useEvent from "react-use-event-hook";
import {setUserSetting} from "renderer/lib/ipc-sender";
import update from "immutability-helper";
import {UserSettings} from "backend";
import BackendAPI from "../api";

type AddTracksResultType = {
  existingIds: number[];
  invalidPaths: string[];
};

function useTracks(userSettings: UserSettings) {
  const [trackIds, setTrackIds] = useState<number[]>([]);
  const [hiddenTrackIds, setHiddenTrackIds] = useState<number[]>([]);
  const [erroredTrackIds, setErroredTrackIds] = useState<number[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(false);
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

  const addTracks = useEvent(
    async (paths: string[], index: number | null = null): Promise<AddTracksResultType> => {
      try {
        setIsLoading(true);
        const idsOfInputPaths = await Promise.all(paths.map(BackendAPI.findIdByPath));
        const newPaths = paths.filter((_, i) => idsOfInputPaths[i] === -1);
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
          setTrackIds((prevTrackIds) => {
            if (index === null) {
              return prevTrackIds.concat(addedIds);
            }
            const newTrackIds = [...prevTrackIds];
            newTrackIds.splice(index, 0, ...addedIds);
            return newTrackIds;
          });
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
    },
  );

  const reloadTracks = useEvent(async (ids: number[]) => {
    try {
      setIsLoading(true);
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
      setIsLoading(false);
    } catch (err) {
      console.error("Could not refresh tracks", err);
    }
  });

  const ignoreError = useEvent((erroredId: number) => {
    setErroredTrackIds((prevErroredTrackIds) => difference(prevErroredTrackIds, [erroredId]));
  });

  const removeTracks = useEvent((ids: number[]) => {
    try {
      setIsLoading(true);
      BackendAPI.removeTracks(ids);
      setTrackIds((prevTrackIds) => difference(prevTrackIds, ids));
      setErroredTrackIds((prevErroredTrackIds) => difference(prevErroredTrackIds, ids));

      addToWaitingIds(ids);
    } catch (err) {
      console.error("Could not remove track", err);
      alert("Could not remove track");
    }
  });

  const hideTracks = useEvent((dragId: number, ids: number[]) => {
    const newTrackIds = trackIds.filter((id) => !ids.includes(id));
    const dragIndex = newTrackIds.indexOf(dragId);
    setTimeout(() => {
      setTrackIds(newTrackIds);
      setHiddenTrackIds(ids);
    });
    return dragIndex;
  });

  const changeTrackOrder = useEvent((dragIndex: number, hoverIndex: number) =>
    setTrackIds((prevTrackOrder) =>
      update(prevTrackOrder, {
        $splice: [
          [dragIndex, 1],
          [hoverIndex, 0, prevTrackOrder[dragIndex]],
        ],
      }),
    ),
  );

  const showHiddenTracks = useEvent((hoverIndex: number) => {
    setTrackIds((prevTrackIds) =>
      update(prevTrackIds, {$splice: [[hoverIndex + 1, 0, ...hiddenTrackIds]]}),
    );
    setHiddenTrackIds([]);
  });

  const setSpecSetting = useEvent(async (v: SpecSetting) => {
    setIsLoading(true);
    await BackendAPI.setSpecSetting(v);
    const specSetting = BackendAPI.getSpecSetting();
    setCurrentSpecSetting(specSetting);
    setUserSetting("specSetting", specSetting);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
    setIsLoading(false);
  });

  const setBlendAndSetUserSetting = useEvent((v: number) => {
    setBlend(v);
    setUserSetting("blend", v);
  });

  const setdBRange = useEvent(async (v: number) => {
    setIsLoading(true);
    await BackendAPI.setdBRange(v);
    const dBRange = await BackendAPI.getdBRange();
    setCurrentdBRange(dBRange);
    setUserSetting("dBRange", dBRange);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
    setIsLoading(false);
  });

  const setCommonGuardClipping = useEvent(async (v: GuardClippingMode) => {
    setIsLoading(true);
    await BackendAPI.setCommonGuardClipping(v);
    const commonGuardClipping = BackendAPI.getCommonGuardClipping();
    setCurrentCommonGuardClipping(commonGuardClipping);
    setUserSetting("commonGuardClipping", commonGuardClipping);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
    setIsLoading(false);
  });

  const setCommonNormalize = useEvent(async (v: NormalizeTarget) => {
    setIsLoading(true);
    await BackendAPI.setCommonNormalize(v);
    const commonNormalize = BackendAPI.getCommonNormalize();
    setCurrentCommonNormalize(commonNormalize);
    setUserSetting("commonNormalize", commonNormalize);
    setNeedRefreshTrackIdChArr(Array.from(trackIdChMap.values()).flat());
    setIsLoading(false);
  });

  const finishRefreshTracks = useEvent(() => {
    setNeedRefreshTrackIdChArr([]);
  });

  return {
    trackIds,
    hiddenTrackIds,
    erroredTrackIds,
    trackIdChMap,
    isLoading,
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
    hideTracks,
    changeTrackOrder,
    showHiddenTracks,
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
