import React, {forwardRef, useEffect, useImperativeHandle, useMemo, useRef} from "react";
import type {Identifier, XYCoord} from "dnd-core";
import {useDrag, useDrop} from "react-dnd";
import {getEmptyImage} from "react-dnd-html5-backend";
import {showTrackContextMenu} from "../../lib/ipc-sender";
import TrackSummary from "./TrackSummary";
import styles from "./TrackInfo.module.scss";
import {CHANNEL, VERTICAL_AXIS_PADDING} from "../constants/tracks";
import {DndItemTypes} from "./ItemTypes";

const MemoizedTrackSummary = React.memo(TrackSummary);

type TrackInfoProps = {
  id: number;
  index: number;
  trackIdChArr: IdChArr;
  selectedTrackIds: number[];
  trackSummary: TrackSummaryData;
  channelHeight: number;
  imgHeight: number;
  isSelected: boolean;
  onMouseDown: (e: React.MouseEvent) => void;
  hideImage: (id: number) => void;
  hideTracks: (dragId: number, ids: number[]) => number;
  onDnd: (dragIndex: number, hoverIndex: number) => void;
  showHiddenTracks: (hoverIndex: number) => void;
  showHiddenImage: () => void;
};

interface DragItem {
  index: number;
  id: string;
  type: string;
}

const TrackInfo = forwardRef((props: TrackInfoProps, ref) => {
  const {
    id,
    index,
    trackIdChArr: trackIdCh,
    selectedTrackIds,
    trackSummary,
    channelHeight,
    imgHeight,
    isSelected,
    onMouseDown,
    hideTracks,
    hideImage,
    onDnd,
    showHiddenTracks,
    showHiddenImage,
  } = props;
  const trackInfoElem = useRef<HTMLDivElement>(null);

  const imperativeHandleRef = useRef<TrackInfoElement>({
    getBoundingClientRect: () => trackInfoElem.current?.getBoundingClientRect() ?? null,
    scrollIntoView: (alignToTop: boolean) =>
      trackInfoElem.current?.scrollIntoView({
        behavior: "smooth",
        block: alignToTop ? "start" : "end",
        inline: "nearest",
      }),
  });
  useImperativeHandle(ref, () => imperativeHandleRef.current, []);

  const style = useMemo(
    () => ({
      margin: `${VERTICAL_AXIS_PADDING}px 0`,
      height: channelHeight * trackIdCh.length - 2 * VERTICAL_AXIS_PADDING,
    }),
    [channelHeight, trackIdCh],
  );
  const channels = useMemo(
    () =>
      trackIdCh.map((idChStr, ch) => {
        return (
          <div key={idChStr} className={styles.ch} style={{height: imgHeight}}>
            <span>{CHANNEL[trackIdCh.length][ch] || ""}</span>
          </div>
        );
      }),
    [trackIdCh, imgHeight],
  );
  const trackSummaryChild = useMemo(
    () => (
      <MemoizedTrackSummary
        className={styles.TrackSummary}
        data={trackSummary}
        chCount={trackIdCh.length}
      />
    ),
    [trackSummary, trackIdCh],
  );

  const [{handlerId}, drop] = useDrop<DragItem, void, {handlerId: Identifier | null}>(
    {
      accept: DndItemTypes.TRACK,
      collect(monitor) {
        return {
          handlerId: monitor.getHandlerId(),
        };
      },
      hover(item: DragItem, monitor) {
        if (!trackInfoElem.current) return;

        const dragIndex = item.index;
        const hoverIndex = index;

        // Don't replace items with themselves
        if (dragIndex === hoverIndex) return;

        // Determine rectangle on screen
        const hoverBoundingRect = trackInfoElem.current?.getBoundingClientRect();

        // Get vertical middle
        const hoverMiddleY = (hoverBoundingRect.bottom - hoverBoundingRect.top) / 2;

        // Determine mouse position
        const clientOffset = monitor.getClientOffset();

        // Get pixels to the top
        const hoverClientY = (clientOffset as XYCoord).y - hoverBoundingRect.top;

        // Only perform the move when the mouse has crossed half of the items height
        // When dragging downwards, only move when the cursor is below 50%
        // When dragging upwards, only move when the cursor is above 50%

        // Dragging downwards
        if (dragIndex < hoverIndex && hoverClientY < hoverMiddleY) return;

        // Dragging upwards
        if (dragIndex > hoverIndex && hoverClientY > hoverMiddleY) return;

        // Time to actually perform the action
        onDnd(dragIndex, hoverIndex);

        // Note: we're mutating the monitor item here!
        // Generally it's better to avoid mutations,
        // but it's good here for the sake of performance
        // to avoid expensive index searches.
        item.index = hoverIndex;
      },
    },
    [index, onDnd],
  );

  const [{isDragging}, drag, preview] = useDrag(
    {
      type: DndItemTypes.TRACK,
      item: () => {
        const hiddenIds = selectedTrackIds.filter((selectedId) => id !== selectedId);
        hideImage(id);
        return {
          id,
          index: hideTracks(id, hiddenIds),
          trackSummaryChild,
          style,
          channels,
          width: trackInfoElem.current?.clientWidth ?? 0,
          numDragging: selectedTrackIds.length,
        };
      },
      isDragging: (monitor) => {
        return id === monitor.getItem().id;
      },
      end: (item) => {
        showHiddenTracks(item.index);
        showHiddenImage();
      },
      collect: (monitor) => ({
        isDragging: monitor.isDragging(),
      }),
    },
    [id, selectedTrackIds, trackSummaryChild, style, channels],
  );

  drag(drop(trackInfoElem));
  useEffect(() => {
    preview(getEmptyImage(), {captureDraggingState: true});
  }, [preview]);

  return (
    <div
      ref={trackInfoElem}
      role="presentation"
      className={`${styles.TrackInfo} ${isSelected ? styles.selected : ""}`}
      onMouseDown={onMouseDown}
      onContextMenu={(e) => {
        e.preventDefault();
        showTrackContextMenu();
      }}
      style={{opacity: isDragging ? 0 : 1, ...style}}
      data-handler-id={handlerId}
    >
      {trackSummaryChild}
      <div className={styles.channels}>{channels}</div>
    </div>
  );
});
TrackInfo.displayName = "TrackInfo";

export default TrackInfo;
