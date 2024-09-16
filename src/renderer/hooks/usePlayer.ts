import {ipcRenderer} from "electron";
import {RefObject, useEffect, useRef, useState} from "react";
import {PLAY_BIG_JUMP_SEC, PLAY_JUMP_SEC} from "main/constants";
import useEvent from "react-use-event-hook";
import {useHotkeys} from "react-hotkeys-hook";
import BackendAPI from "../api";
import {showPlayOrPauseMenu} from "../lib/ipc-sender";

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

function usePlayer(selectedTrackId: number) {
  const [_currentPlayingTrack, setCurrentPlayingTrack] = useState<number>(-1);
  const [isPlaying, setIsPlaying] = useState<boolean>(false);

  const positionSecRef = useRef<number>(0);
  const selectSecRef = useRef<number>(0);
  const setSelectSec = useEvent((sec: number) => {
    selectSecRef.current = Math.min(Math.max(sec, 0), BackendAPI.getLongestTrackLengthSec());
  });

  const requestRef = useRef<number>(0);

  const updatePlayerStates = useEvent(() => {
    const {isPlaying: newIsPlaying, positionSec, err} = BackendAPI.getPlayerState();
    if (err) console.error(err);
    if (isPlaying !== newIsPlaying) setIsPlaying(newIsPlaying);
    positionSecRef.current = positionSec;
    requestRef.current = requestAnimationFrame(updatePlayerStates);
  });

  useEffect(() => {
    requestRef.current = requestAnimationFrame(updatePlayerStates);
    return () => cancelAnimationFrame(requestRef.current);
  }, [updatePlayerStates]);

  const setPlayingTrack = useEvent((trackId: number) => {
    setCurrentPlayingTrack((current) => {
      if (current === trackId) return current;
      if (trackId >= 0) BackendAPI.setTrackPlayer(trackId);
      return trackId;
    });
  });

  const togglePlay = useEvent(async () => {
    if (isPlaying) {
      await BackendAPI.pausePlayer();
    } else {
      BackendAPI.seekPlayer(selectSecRef.current);
      await BackendAPI.resumePlayer();
    }
  });

  useEffect(() => {
    setPlayingTrack(selectedTrackId);
    if (selectedTrackId < 0) {
      setSelectSec(0);
      BackendAPI.seekPlayer(0);
    }
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
