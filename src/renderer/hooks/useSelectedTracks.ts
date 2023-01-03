import React, {useCallback, useState} from "react";

export default function useSelectedTracks() {
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

    if (!(nextSelectedTrackId === null)) {
      setSelectedTrackIds([nextSelectedTrackId]);
    }
  }, []);

  const selectTrackAfterRemoveTracks = useCallback(
    (prevTrackIds: number[], newTrackIds: number[]) => {
      let nextSelectedTrackIndex = newTrackIds.length;
      selectedTrackIds.forEach((id) => {
        nextSelectedTrackIndex = Math.min(nextSelectedTrackIndex, prevTrackIds.indexOf(id));
      });
      const nextSelectedTrackId =
        newTrackIds[nextSelectedTrackIndex] ?? newTrackIds[newTrackIds.length - 1];

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
