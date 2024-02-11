import {useState} from "react";
import useEvent from "react-use-event-hook";
import {isCommand} from "renderer/utils/commandKey";

function useSelectedTracks() {
  const [selectedTrackIds, setSelectedTrackIds] = useState<number[]>([]);
  const [pivotId, setPivotId] = useState<number>(0);

  const selectTrack = useEvent((e: MouseOrKeyboardEvent, id: number, trackIds: number[]) => {
    e.preventDefault();

    if (isCommand(e)) {
      const selectedIndexOfId = selectedTrackIds.indexOf(id);
      if (selectedIndexOfId === -1) {
        // add id
        setPivotId(id);
        setSelectedTrackIds(selectedTrackIds.concat([id]));
        return;
      }
      if (selectedTrackIds.length === 1) return;
      // remove id
      const newSelected = selectedTrackIds
        .slice(0, selectedIndexOfId)
        .concat(selectedTrackIds.slice(selectedIndexOfId + 1, undefined));
      if (pivotId === id) setPivotId(newSelected[newSelected.length - 1]);
      setSelectedTrackIds(newSelected);
      return;
    }
    if (e.shiftKey) {
      if (id === selectedTrackIds[selectedTrackIds.length - 1]) return;
      // const indexOfRecent = trackIds.indexOf(selectedTrackIds[selectedTrackIds.length - 1]);
      const indexOfId = trackIds.indexOf(id);
      const indexOfPivot = trackIds.indexOf(pivotId);
      // remove ids that added after the pivot (by shift+select)
      let newSelected = selectedTrackIds.slice(0, selectedTrackIds.indexOf(pivotId) + 1);

      // add "one after pivot" ~ id
      let addingIds: number[];
      if (indexOfId > indexOfPivot) addingIds = trackIds.slice(indexOfPivot + 1, indexOfId + 1);
      else addingIds = trackIds.slice(indexOfId, indexOfPivot);
      // if newSelected has some of addingIds, remove them first
      newSelected = newSelected.filter((selectedId) => !addingIds.includes(selectedId));
      newSelected.push(...addingIds);
      setSelectedTrackIds(newSelected);
      return;
    }
    // with nothing pressed
    if (selectedTrackIds.length === 1 && selectedTrackIds[0] === id) {
      return;
    }
    setPivotId(id);
    setSelectedTrackIds([id]);
  });

  const selectTrackAfterAddTracks = useEvent((prevTrackIds: number[], newTrackIds: number[]) => {
    const nextSelectedTrackIndex = prevTrackIds.length;
    if (newTrackIds.length > nextSelectedTrackIndex) {
      const nextSelectedTrackId = newTrackIds[nextSelectedTrackIndex];
      setPivotId(nextSelectedTrackId);
      setSelectedTrackIds([nextSelectedTrackId]);
    }
  });

  const selectTrackAfterRemoveTracks = useEvent((prevTrackIds: number[], newTrackIds: number[]) => {
    if (newTrackIds.length === 0) {
      setPivotId(-1);
      setSelectedTrackIds([]);
      return;
    }
    let nextSelectedTrackIndex = newTrackIds.length - 1;
    selectedTrackIds.forEach((id) => {
      nextSelectedTrackIndex = Math.min(nextSelectedTrackIndex, prevTrackIds.indexOf(id));
    });
    const nextSelectedTrackId = newTrackIds[nextSelectedTrackIndex];

    if (nextSelectedTrackId !== selectedTrackIds[0]) {
      setPivotId(nextSelectedTrackId);
      setSelectedTrackIds([nextSelectedTrackId]);
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
