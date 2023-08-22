import React, {useCallback, useState} from "react";

function useSelectedTracks() {
  const [selectedTrackIds, setSelectedTrackIds] = useState<number[]>([]);

  const selectTrack = useCallback((e: React.MouseEvent, id: number) => {
    e.preventDefault();

    // with nothing pressed
    setSelectedTrackIds([id]);
  }, []);

  const selectTrackAfterAddTracks = useCallback((prevTrackIds: number[], newTrackIds: number[]) => {
    const nextSelectedTrackIndex = prevTrackIds.length;
    if (newTrackIds.length > nextSelectedTrackIndex) {
      const nextSelectedTrackId = newTrackIds[nextSelectedTrackIndex];
      setSelectedTrackIds([nextSelectedTrackId]);
    }
  }, []);

  const selectTrackAfterRemoveTracks = useCallback(
    (prevTrackIds: number[], newTrackIds: number[]) => {
      if (newTrackIds.length) {
        let nextSelectedTrackIndex = newTrackIds.length - 1;
        selectedTrackIds.forEach((id) => {
          nextSelectedTrackIndex = Math.min(nextSelectedTrackIndex, prevTrackIds.indexOf(id));
        });
        const nextSelectedTrackId = newTrackIds[nextSelectedTrackIndex];

        setSelectedTrackIds([nextSelectedTrackId]);
      } else {
        setSelectedTrackIds([]);
      }
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
