import React from "react";
import type {XYCoord} from "react-dnd";
import {useDragLayer} from "react-dnd";
import DndItemTypes from "../constants/DndItemTypes";
import styles from "./TrackInfo.module.scss";

function getItemStyles(initialOffset: XYCoord | null, currentOffset: XYCoord | null) {
  if (!initialOffset || !currentOffset) return {display: "none"};

  const transform = `translate(${initialOffset.x}px, ${currentOffset.y}px)`;
  return {transform, WebkitTransform: transform};
}

function TrackInfoDragLayer() {
  const {itemType, isDragging, item, initialOffset, currentOffset} = useDragLayer((monitor) => ({
    item: monitor.getItem(),
    itemType: monitor.getItemType(),
    initialOffset: monitor.getInitialSourceClientOffset(),
    currentOffset: monitor.getSourceClientOffset(),
    isDragging: monitor.isDragging(),
  }));

  function renderItem() {
    switch (itemType) {
      case DndItemTypes.TRACK:
        return (
          <div
            className={`${styles.TrackInfo} ${styles.selected}`}
            style={{width: item.width, ...item.style}}
          >
            {item.trackSummaryChild}
            <div className={styles.channels}>{item.channels}</div>
            {item.numDragging > 1 ? (
              <span style={{position: "absolute", right: 0, bottom: 0}}>{item.numDragging}</span>
            ) : (
              ""
            )}
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
    </div>
  );
}

export default React.memo(TrackInfoDragLayer);
