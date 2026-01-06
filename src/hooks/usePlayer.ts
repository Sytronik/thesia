import { RefObject, useContext, useEffect, useRef, useState } from "react";
import useEvent from "react-use-event-hook";
import { useHotkeys } from "react-hotkeys-hook";
import BackendAPI from "../api";
import { listenJumpPlayer, listenRewindToFront, listenTogglePlay } from "../api";
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
  const [currentPlayingTrack, setCurrentPlayingTrack] = useState<number>(-1);
  const [isPlaying, setIsPlaying] = useState<boolean>(false);
  const deviceErrorRef = useRef<string | null>(null);

  const positionSecRef = useRef<number>(0);
  const selectSecRef = useRef<number>(0);
  const setSelectSec = useEvent((sec: number) => {
    selectSecRef.current = Math.min(Math.max(sec, 0), maxTrackSec);
  });

  const requestRef = useRef<number>(0);
  const updatePlayerStatesRef = useRef<(() => Promise<void>) | null>(null);

  const updatePlayerStates = useEvent(async () => {
    // const start = performance.now();
    const { isPlaying: newIsPlaying, positionSec, err } = await BackendAPI.getPlayerState();
    // const end = performance.now();
    // console.log(`Execution time: ${end - start} ms`);
    if (err) {
      if (deviceErrorRef.current === null) console.error(err);
      deviceErrorRef.current = err;
    } else {
      if (deviceErrorRef.current !== null) console.log("Error resolved");
      deviceErrorRef.current = null;
    }
    if (isPlaying !== newIsPlaying) setIsPlaying(newIsPlaying);
    positionSecRef.current = positionSec;
    if (updatePlayerStatesRef.current)
      requestRef.current = requestAnimationFrame(updatePlayerStatesRef.current);
  });

  useEffect(() => {
    updatePlayerStatesRef.current = updatePlayerStates;
    requestRef.current = requestAnimationFrame(updatePlayerStatesRef.current);
    return () => cancelAnimationFrame(requestRef.current);
  }, [updatePlayerStates]);

  const setPlayingTrack = useEvent(async (trackId: number) => {
    if (trackId === currentPlayingTrack || trackId < 0) return;
    await BackendAPI.setTrackPlayer(trackId);
    setCurrentPlayingTrack(trackId);
  });

  const togglePlay = useEvent(async () => {
    if (isPlaying) {
      await BackendAPI.pausePlayer();
    } else if (selectedTrackId >= 0) {
      await BackendAPI.seekPlayer(selectSecRef.current);
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
      await BackendAPI.seekPlayer((positionSecRef.current ?? 0) + jumpSec);
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
    if (isPlaying) await BackendAPI.seekPlayer(0);
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
    seek: BackendAPI.seekPlayer,
    jump,
    rewindToFront,
  } as Player;
}

export default usePlayer;
