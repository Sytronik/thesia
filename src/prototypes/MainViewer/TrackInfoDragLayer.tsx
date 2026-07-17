import React from "react";
import type { XYCoord } from "react-dnd";
import { useDragLayer } from "react-dnd";
import DndItemTypes from "../constants/DndItemTypes";
import styles from "./TrackInfo.module.scss";

type TrackDragItem = {
  width: number;
  style: React.CSSProperties;
  trackSummaryChild: React.ReactNode;
  channels: React.ReactNode;
  numDragging: number;
};

function getItemStyles(initialOffset: XYCoord | null, currentOffset: XYCoord | null) {
  if (!initialOffset || !currentOffset) return { display: "none" };

  const transform = `translate(${initialOffset.x}px, ${currentOffset.y}px)`;
  return { transform, WebkitTransform: transform };
}

function getDragCountStyles(clientOffset: XYCoord | null): React.CSSProperties {
  if (!clientOffset) return { display: "none" };

  const placeToLeft = clientOffset.x > window.innerWidth - 144;
  const placeAbove = clientOffset.y > window.innerHeight - 56;
  const x = placeToLeft ? "calc(-100% - 14px)" : "18px";
  const y = placeAbove ? "calc(-100% - 14px)" : "18px";

  return {
    left: clientOffset.x,
    top: clientOffset.y,
    transform: `translate(${x}, ${y})`,
  };
}

function TrackInfoDragLayer() {
  const { itemType, isDragging, item, initialOffset, currentOffset, clientOffset } = useDragLayer(
    (monitor) => ({
      item: monitor.getItem<TrackDragItem>(),
      itemType: monitor.getItemType(),
      initialOffset: monitor.getInitialSourceClientOffset(),
      currentOffset: monitor.getSourceClientOffset(),
      clientOffset: monitor.getClientOffset(),
      isDragging: monitor.isDragging(),
    }),
  );

  function renderItem() {
    switch (itemType) {
      case DndItemTypes.TRACK:
        return (
          <div
            className={`${styles.TrackInfo} ${styles.selected}`}
            style={{ width: item.width, ...item.style }}
          >
            {item.trackSummaryChild}
            <div className={styles.channels}>{item.channels}</div>
          </div>
        );
      default:
        return null;
    }
  }

  if (!isDragging) return null;

  return (
    <div
      style={{
        position: "fixed",
        pointerEvents: "none",
        zIndex: 100,
        left: 0,
        top: 0,
        width: "100%",
        height: "100%",
        backgroundColor: "transparent",
      }}
    >
      <div style={getItemStyles(initialOffset, currentOffset)}>{renderItem()}</div>
      {itemType === DndItemTypes.TRACK && item.numDragging > 1 ? (
        <div className={styles.dragCountPosition} style={getDragCountStyles(clientOffset)}>
          <div className={styles.dragCountBadge}>
            <svg className={styles.dragCountIcon} viewBox="0 0 18 18" aria-hidden="true">
              <rect x="2.5" y="2.5" width="10" height="7" rx="2" />
              <rect x="5.5" y="8.5" width="10" height="7" rx="2" />
            </svg>
            <span className={styles.dragCountNumber}>{item.numDragging}</span>
            <span className={styles.dragCountLabel}>tracks</span>
          </div>
        </div>
      ) : null}
    </div>
  );
}

export default React.memo(TrackInfoDragLayer);
