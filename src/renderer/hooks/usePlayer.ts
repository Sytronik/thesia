import {RefObject, useEffect, useRef, useState} from "react";
import useEvent from "react-use-event-hook";
import BackendAPI from "renderer/api";

export type Player = {
  isPlaying: boolean;
  positionSecRef: RefObject<number>;
  togglePlay: () => Promise<void>;
  seek: (sec: number) => Promise<void>;
};

function usePlayer(selectedTrackId: number) {
  const [_currentPlayingTrack, setCurrentPlayingTrack] = useState<number>(-1);
  const [isPlaying, setIsPlaying] = useState<boolean>(false);

  const positionSecRef = useRef<number>(0);
  const requestRef = useRef<number>(0);

  const updatePlayerStates = useEvent(() => {
    const {isPlaying: newIsPlaying, positionSec} = BackendAPI.getPlayerStatus();
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
    if (isPlaying) await BackendAPI.pausePlayer();
    else await BackendAPI.resumePlayer();
  });

  useEffect(() => {
    setPlayingTrack(selectedTrackId);
  }, [selectedTrackId, setPlayingTrack]);

  return {
    isPlaying,
    positionSecRef,
    togglePlay,
    seek: BackendAPI.seekPlayer,
  } as Player;
}

export default usePlayer;
