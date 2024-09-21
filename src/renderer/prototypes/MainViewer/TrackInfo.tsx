import React, {forwardRef, useImperativeHandle, useRef} from "react";
import type {Identifier, XYCoord} from "dnd-core";
import {useDrag, useDrop} from "react-dnd";
import {ItemTypes} from "./ItemTypes";
import {showTrackContextMenu} from "../../lib/ipc-sender";
import TrackSummary from "./TrackSummary";
import styles from "./TrackInfo.module.scss";
import {CHANNEL, VERTICAL_AXIS_PADDING} from "../constants/tracks";

const MemoizedTrackSummary = React.memo(TrackSummary);

type TrackInfoProps = {
  id: number;
  index: number;
  trackIdChArr: IdChArr;
  trackSummary: TrackSummaryData;
  channelHeight: number;
  imgHeight: number;
  isSelected: boolean;
  onMouseDown: (e: React.MouseEvent) => void;
  onDnd: (dragIndex: number, hoverIndex: number) => void;
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
    trackSummary,
    channelHeight,
    imgHeight,
    isSelected,
    onMouseDown,
    onDnd,
  } = props;
  const trackInfoElem = useRef<HTMLDivElement>(null);

  const channels = trackIdCh.map((idChStr, ch) => {
    return (
      <div key={idChStr} className={styles.ch} style={{height: imgHeight}}>
        <span>{CHANNEL[trackIdCh.length][ch] || ""}</span>
      </div>
    );
  });

  const imperativeHandleRef = useRef<TrackInfoElement>({
    getBoundingClientRect: () => trackInfoElem.current?.getBoundingClientRect() ?? null,
  });
  useImperativeHandle(ref, () => imperativeHandleRef.current, []);

  const [{handlerId}, drop] = useDrop<DragItem, void, {handlerId: Identifier | null}>(
    {
      accept: ItemTypes.TRACK,
      collect(monitor) {
        return {
          handlerId: monitor.getHandlerId(),
        };
      },
      hover(item: DragItem, monitor) {
        if (!trackInfoElem.current) {
          return;
        }
        const dragIndex = item.index;
        const hoverIndex = index;

        // Don't replace items with themselves
        if (dragIndex === hoverIndex) {
          return;
        }

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
        if (dragIndex < hoverIndex && hoverClientY < hoverMiddleY) {
          return;
        }

        // Dragging upwards
        if (dragIndex > hoverIndex && hoverClientY > hoverMiddleY) {
          return;
        }

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

  const [{isDragging}, drag] = useDrag(
    {
      type: ItemTypes.TRACK,
      item: () => {
        return {id, index};
      },
      collect: (monitor: any) => ({
        isDragging: monitor.isDragging(),
      }),
    },
    [id, index],
  );

  const opacity = isDragging ? 0 : 1;
  drag(drop(trackInfoElem));

  return (
    <div
      ref={trackInfoElem}
      role="presentation"
      className={`${styles.TrackInfo} ${isSelected ? styles.selected : ""}`}
      onMouseDown={onMouseDown}
      onContextMenu={(e) => {
        e.preventDefault();
        showTrackContextMenu();
      }} // TODO: if (!isSelected), show highlight instead
      style={{
        margin: `${VERTICAL_AXIS_PADDING}px 0`,
        height: channelHeight * trackIdCh.length - 2 * VERTICAL_AXIS_PADDING,
        opacity,
      }}
      data-handler-id={handlerId}
    >
      <MemoizedTrackSummary className={styles.TrackSummary} data={trackSummary} />
      <div className={styles.channels}>{channels}</div>
    </div>
  );
});
TrackInfo.displayName = "TrackInfo";

export default TrackInfo;
