import React, {useCallback, useState} from "react";
import {last, isNil} from "renderer/utils/arrayUtils";

function useSelectedTracks() {
  const [selectedTrackIds, setSelectedTrackIds] = useState<number[]>([]);

  const selectTrack = useCallback(
    (e: React.MouseEvent, id: number) => {
      e.preventDefault();

      // with nothing pressed
      setSelectedTrackIds([id]);
    },
    [setSelectedTrackIds],
  );

  const selectTrackAfterAddTracks = useCallback((prevTrackIds: number[], newTrackIds: number[]) => {
    const nextSelectedTrackIndex = prevTrackIds.length;
    const nextSelectedTrackId = newTrackIds[nextSelectedTrackIndex];

    if (!isNil(nextSelectedTrackId)) {
      setSelectedTrackIds([nextSelectedTrackId]);
    }
  }, []);

  const selectTrackAfterRemoveTracks = useCallback(
    (prevTrackIds: number[], newTrackIds: number[]) => {
      let nextSelectedTrackIndex = newTrackIds.length;
      selectedTrackIds.forEach((id) => {
        nextSelectedTrackIndex = Math.min(nextSelectedTrackIndex, prevTrackIds.indexOf(id));
      });
      const nextSelectedTrackId = newTrackIds[nextSelectedTrackIndex] ?? last(newTrackIds);

      setSelectedTrackIds([nextSelectedTrackId]);
    },
    [selectedTrackIds],
  );

  return {
    selectedTrackIds,
    selectTrack,
    selectTrackAfterAddTracks,
    selectTrackAfterRemoveTracks,
  };
}

export default useSelectedTracks;
