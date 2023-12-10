import React, {useState} from "react";
import useEvent from "react-use-event-hook";

function useSelectedTracks() {
  const [selectedTrackIds, setSelectedTrackIds] = useState<number[]>([]);

  const selectTrack = useEvent((e: Event | React.MouseEvent, id: number) => {
    e.preventDefault();

    // with nothing pressed
    setSelectedTrackIds((current) => (id !== current[0] ? [id] : current));
  });

  const selectTrackAfterAddTracks = useEvent((prevTrackIds: number[], newTrackIds: number[]) => {
    const nextSelectedTrackIndex = prevTrackIds.length;
    if (newTrackIds.length > nextSelectedTrackIndex) {
      const nextSelectedTrackId = newTrackIds[nextSelectedTrackIndex];
      setSelectedTrackIds([nextSelectedTrackId]);
    }
  });

  const selectTrackAfterRemoveTracks = useEvent((prevTrackIds: number[], newTrackIds: number[]) => {
    if (newTrackIds.length) {
      let nextSelectedTrackIndex = newTrackIds.length - 1;
      selectedTrackIds.forEach((id) => {
        nextSelectedTrackIndex = Math.min(nextSelectedTrackIndex, prevTrackIds.indexOf(id));
      });
      const nextSelectedTrackId = newTrackIds[nextSelectedTrackIndex];

      if (nextSelectedTrackId !== selectedTrackIds[0]) setSelectedTrackIds([nextSelectedTrackId]);
    } else {
      setSelectedTrackIds([]);
    }
  });

  return {
    selectedTrackIds,
    selectTrack,
    selectTrackAfterAddTracks,
    selectTrackAfterRemoveTracks,
  };
}

export default useSelectedTracks;
