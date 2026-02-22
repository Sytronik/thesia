import { RefObject, useContext, useEffect, useRef, useState } from "react";
import useEvent from "react-use-event-hook";
import { useHotkeys } from "react-hotkeys-hook";
import BackendAPI from "../api";
import {
  listenJumpPlayer,
  listenPlayerStateChanged,
  listenRewindToFront,
  listenTogglePlay,
} from "../api";
import { BackendConstantsContext } from "../contexts";

export type Player = {
  isPlaying: boolean;
  positionSecRef: RefObject<number>;
  selectSecRef: RefObject<number>;
  setSelectSec: (sec: number) => void;
  togglePlay: () => Promise<void>;
  seek: (sec: number) => Promise<void>;
  jump: (jumpSec: number) => Promise<void>;
  rewindToFront: () => Promise<void>;
};

function usePlayer(selectedTrackId: number, maxTrackSec: number) {
  const { PLAY_JUMP_SEC, PLAY_BIG_JUMP_SEC } = useContext(BackendConstantsContext);
  const TRACK_SWITCH_SEEK_TTL_MS = 1000;
  const [currentPlayingTrack, setCurrentPlayingTrack] = useState<number>(-1);
  const [isPlaying, setIsPlaying] = useState<boolean>(false);
  const deviceErrorRef = useRef<string | null>(null);

  const positionSecRef = useRef<number>(0);
  const anchorPositionSecRef = useRef<number>(0);
  const anchorEventTimeMsRef = useRef<number>(0);
  const pendingTrackStartSecRef = useRef<number | null>(null);
  const pendingTrackStartEventMsRef = useRef<number>(0);
  const selectSecRef = useRef<number>(0);
  const setSelectSec = useEvent((sec: number) => {
    selectSecRef.current = Math.min(Math.max(sec, 0), maxTrackSec);
  });

  const requestRef = useRef<number>(0);
  const updatePositionRef = useRef<(() => void) | null>(null);

  const updatePosition = useEvent(() => {
    let positionSec = anchorPositionSecRef.current;
    if (isPlaying) {
      const elapsedSec = Math.max(0, Date.now() - anchorEventTimeMsRef.current) / 1000;
      positionSec += elapsedSec;
    }
    positionSecRef.current = Math.min(Math.max(positionSec, 0), maxTrackSec);
    if (updatePositionRef.current) requestRef.current = requestAnimationFrame(updatePositionRef.current);
  });

  useEffect(() => {
    updatePositionRef.current = updatePosition;
    requestRef.current = requestAnimationFrame(updatePositionRef.current);
    return () => cancelAnimationFrame(requestRef.current);
  }, [updatePosition]);

  useEffect(() => {
    const promiseUnlisten = listenPlayerStateChanged(
      ({ isPlaying: nextIsPlaying, positionSec, eventTimeMs, err }) => {
        const clampedPositionSec = Math.min(Math.max(positionSec, 0), maxTrackSec);
        anchorPositionSecRef.current = clampedPositionSec;
        anchorEventTimeMsRef.current = Number.isFinite(eventTimeMs) ? eventTimeMs : Date.now();
        positionSecRef.current = clampedPositionSec;
        setIsPlaying(nextIsPlaying);

        if (err) {
          if (deviceErrorRef.current === null) console.error(err);
          deviceErrorRef.current = err;
        } else {
          if (deviceErrorRef.current !== null) console.log("Error resolved");
          deviceErrorRef.current = null;
        }
      },
    );
    return () => {
      promiseUnlisten.then((unlistenFn) => unlistenFn());
    };
  }, [maxTrackSec]);

  const seek = useEvent(async (sec: number) => {
    const clampedSec = Math.min(Math.max(sec, 0), maxTrackSec);
    pendingTrackStartSecRef.current = clampedSec;
    pendingTrackStartEventMsRef.current = Date.now();
    await BackendAPI.seekPlayer(clampedSec);
  });

  const setPlayingTrack = useEvent(async (trackId: number) => {
    if (trackId === currentPlayingTrack || trackId < 0) return;

    const now = Date.now();
    const pendingSec = pendingTrackStartSecRef.current;
    const usePendingSec =
      pendingSec !== null && now - pendingTrackStartEventMsRef.current <= TRACK_SWITCH_SEEK_TTL_MS;

    let startSec = usePendingSec ? pendingSec : selectSecRef.current;
    if (!usePendingSec && isPlaying) {
      const elapsedSec = Math.max(0, now - anchorEventTimeMsRef.current) / 1000;
      startSec = anchorPositionSecRef.current + elapsedSec;
    }
    startSec = Math.min(Math.max(startSec, 0), maxTrackSec);

    pendingTrackStartSecRef.current = null;
    pendingTrackStartEventMsRef.current = 0;

    await BackendAPI.setTrackPlayer(trackId, startSec);
    setCurrentPlayingTrack(trackId);
  });

  const togglePlay = useEvent(async () => {
    if (isPlaying) {
      await BackendAPI.pausePlayer();
    } else if (selectedTrackId >= 0) {
      await seek(selectSecRef.current);
      await BackendAPI.resumePlayer();
    }
  });

  useEffect(() => {
    setPlayingTrack(selectedTrackId);
    if (selectedTrackId >= 0) {
      BackendAPI.enableTogglePlayMenu();
      return;
    }
    BackendAPI.seekPlayer(0);
    BackendAPI.pausePlayer();
    BackendAPI.disableTogglePlayMenu();
    setSelectSec(0);
  }, [selectedTrackId, setPlayingTrack, setSelectSec]);

  // Player Hotkeys
  useHotkeys("space", togglePlay, { preventDefault: true }, [togglePlay]);
  useEffect(() => {
    const promiseUnlisten = listenTogglePlay(togglePlay);
    return () => {
      promiseUnlisten.then((unlistenFn) => unlistenFn());
    };
  }, [togglePlay]);

  const jump = useEvent(async (jumpSec: number) => {
    if (isPlaying) {
      await seek((positionSecRef.current ?? 0) + jumpSec);
      return;
    }
    setSelectSec((selectSecRef.current ?? 0) + jumpSec);
  });
  useHotkeys(
    "comma,period,shift+comma,shift+period",
    (_, hotkey) => {
      let jumpSec = hotkey.shift ? PLAY_BIG_JUMP_SEC : PLAY_JUMP_SEC;
      if (hotkey.keys?.join("") === "comma") jumpSec = -jumpSec;
      jump(jumpSec);
    },
    { preventDefault: true },
    [jump],
  );
  useEffect(() => {
    const promiseUnlisten = listenJumpPlayer((mode) => {
      switch (mode) {
        case "fast-forward":
          jump(PLAY_JUMP_SEC);
          break;
        case "rewind":
          jump(-PLAY_JUMP_SEC);
          break;
        case "fast-forward-big":
          jump(PLAY_BIG_JUMP_SEC);
          break;
        case "rewind-big":
          jump(-PLAY_BIG_JUMP_SEC);
          break;
      }
    });
    return () => {
      promiseUnlisten.then((unlistenFn) => unlistenFn());
    };
  }, [jump, PLAY_JUMP_SEC, PLAY_BIG_JUMP_SEC]);

  const rewindToFront = useEvent(async () => {
    if (isPlaying) await seek(0);
    else setSelectSec(0);
  });
  useHotkeys("enter", rewindToFront, { preventDefault: true }, [rewindToFront]);
  useEffect(() => {
    const promiseUnlisten = listenRewindToFront(rewindToFront);
    return () => {
      promiseUnlisten.then((unlistenFn) => unlistenFn());
    };
  }, [rewindToFront]);

  useEffect(() => {
    BackendAPI.showPlayOrPauseMenu(isPlaying);
  }, [isPlaying]);

  return {
    isPlaying,
    positionSecRef,
    selectSecRef,
    setSelectSec,
    togglePlay,
    seek,
    jump,
    rewindToFront,
  } as Player;
}

export default usePlayer;
