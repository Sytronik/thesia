import {ipcRenderer} from "electron";
import {RefObject, useEffect, useRef, useState} from "react";
import {PLAY_BIG_JUMP_SEC, PLAY_JUMP_SEC} from "main/constants";
import useEvent from "react-use-event-hook";
import {useHotkeys} from "react-hotkeys-hook";
import BackendAPI from "../api";
import {disableTogglePlayMenu, enableTogglePlayMenu, showPlayOrPauseMenu} from "../lib/ipc-sender";

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
  const [currentPlayingTrack, setCurrentPlayingTrack] = useState<number>(-1);
  const [isPlaying, setIsPlaying] = useState<boolean>(false);
  const deviceErrorRef = useRef<string | null>(null);

  const positionSecRef = useRef<number>(0);
  const selectSecRef = useRef<number>(0);
  const setSelectSec = useEvent((sec: number) => {
    selectSecRef.current = Math.min(Math.max(sec, 0), maxTrackSec);
  });

  const requestRef = useRef<number>(0);

  const updatePlayerStates = useEvent(() => {
    // const start = performance.now();
    const {isPlaying: newIsPlaying, positionSec, err} = BackendAPI.getPlayerState();
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
    requestRef.current = requestAnimationFrame(updatePlayerStates);
  });

  useEffect(() => {
    requestRef.current = requestAnimationFrame(updatePlayerStates);
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
      enableTogglePlayMenu();
      return;
    }
    BackendAPI.seekPlayer(0);
    BackendAPI.pausePlayer();
    disableTogglePlayMenu();
    setSelectSec(0);
  }, [selectedTrackId, setPlayingTrack, setSelectSec]);

  // Player Hotkeys
  useHotkeys("space", togglePlay, {preventDefault: true}, [togglePlay]);
  useEffect(() => {
    ipcRenderer.on("toggle-play", togglePlay);
    return () => {
      ipcRenderer.removeAllListeners("toggle-play");
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
    [jump],
  );
  useEffect(() => {
    ipcRenderer.on("jump-player", (_, jumpSec) => jump(jumpSec));
    return () => {
      ipcRenderer.removeAllListeners("jump-player");
    };
  }, [jump]);

  const rewindToFront = useEvent(async () => {
    if (isPlaying) await BackendAPI.seekPlayer(0);
    else setSelectSec(0);
  });
  useHotkeys("enter", rewindToFront, {preventDefault: true}, [rewindToFront]);
  useEffect(() => {
    ipcRenderer.on("rewind-to-front", rewindToFront);
    return () => {
      ipcRenderer.removeAllListeners("rewind-to-front");
    };
  }, [rewindToFront]);

  useEffect(() => {
    showPlayOrPauseMenu(isPlaying);
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
