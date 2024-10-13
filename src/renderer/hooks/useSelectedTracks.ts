import {useState} from "react";
import useEvent from "react-use-event-hook";
import {isCommand} from "renderer/utils/osSpecifics";

function useSelectedTracks() {
  const [selectedTrackIds, setSelectedTrackIds] = useState<number[]>([]);
  const [selectionIsAdded, setSelectionIsAdded] = useState<boolean>(false);
  const [pivotId, setPivotId] = useState<number>(0);

  const selectTrack = useEvent((e: MouseOrKeyboardEvent, id: number, trackIds: number[]) => {
    setSelectionIsAdded(false); // by default, consider selection not added

    if (isCommand(e)) {
      const idxInSelectedIds = selectedTrackIds.indexOf(id);
      if (idxInSelectedIds === -1) {
        // add id
        setPivotId(id);
        setSelectedTrackIds(selectedTrackIds.concat([id]));
        setSelectionIsAdded(true);
        return;
      }
      if (selectedTrackIds.length === 1) return;
      // remove id
      const newSelected = selectedTrackIds
        .slice(0, idxInSelectedIds)
        .concat(selectedTrackIds.slice(idxInSelectedIds + 1, undefined));
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
      else addingIds = trackIds.slice(indexOfId, indexOfPivot).reverse();
      // if newSelected has some of addingIds, remove them first
      newSelected = newSelected
        .filter((selectedId) => !addingIds.includes(selectedId))
        .concat(addingIds);
      setSelectedTrackIds(newSelected);
      if (addingIds.length > 0) setSelectionIsAdded(true);
      return;
    }
    // with nothing pressed
    if (selectedTrackIds.length === 1 && selectedTrackIds[0] === id) {
      return;
    }
    setPivotId(id);
    setSelectedTrackIds([id]);
    setSelectionIsAdded(true);
  });

  const selectAllTracks = useEvent((trackIds: number[]) => {
    if (
      trackIds.length === selectedTrackIds.length &&
      trackIds.every((id) => selectedTrackIds.includes(id))
    ) {
      return;
    }
    setPivotId(trackIds[trackIds.length - 1]);
    setSelectedTrackIds(trackIds);
  });

  const selectTrackAfterAddTracks = useEvent((prevTrackIds: number[], newTrackIds: number[]) => {
    const newSelected = newTrackIds.filter((id) => !prevTrackIds.includes(id));
    if (newSelected.length === 0) return;
    setPivotId(newSelected[newSelected.length - 1]);
    setSelectedTrackIds(newSelected);
  });

  const selectTrackAfterRemoveTracks = useEvent((prevTrackIds: number[], newTrackIds: number[]) => {
    if (newTrackIds.length === 0) {
      setPivotId(-1);
      setSelectedTrackIds([]);
      return;
    }
    // check retains
    const newSelected = selectedTrackIds.filter((id) => newTrackIds.includes(id));
    if (newSelected.length > 0) {
      if (!newSelected.includes(pivotId)) setPivotId(newSelected[newSelected.length - 1]);
      setSelectedTrackIds(newSelected);
      return;
    }
    // select the nearest id from the (previous) pivot
    const prevIndexOfPivot = prevTrackIds.indexOf(pivotId);
    for (let i = 1; i < prevTrackIds.length; i += 1) {
      let id = prevTrackIds[prevIndexOfPivot - i];
      if (newTrackIds.includes(id)) {
        setPivotId(id);
        setSelectedTrackIds([id]);
        return;
      }
      id = prevTrackIds[prevIndexOfPivot + i];
      if (newTrackIds.includes(id)) {
        setPivotId(id);
        setSelectedTrackIds([id]);
        return;
      }
    }
    // unreachable, but defensive
    setPivotId(newTrackIds[0]);
    setSelectedTrackIds([newTrackIds[0]]);
  });

  return {
    selectedTrackIds,
    selectionIsAdded,
    selectTrack,
    selectAllTracks,
    selectTrackAfterAddTracks,
    selectTrackAfterRemoveTracks,
  };
}

export default useSelectedTracks;
