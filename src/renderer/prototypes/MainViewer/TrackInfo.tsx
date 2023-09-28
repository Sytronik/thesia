import React from "react";
import {showElectronContextMenu} from "renderer/lib/electron-sender";
import TrackSummary from "./TrackSummary";
import styles from "./TrackInfo.scss";
import {CHANNEL, VERTICAL_AXIS_PADDING} from "../constants";

const MemoizedTrackSummary = React.memo(TrackSummary);

type TrackInfoProps = {
  trackId: number;
  trackIdChArr: IdChArr;
  trackSummary: TrackSummary;
  channelHeight: number;
  imgHeight: number;
  isSelected: boolean;
  selectTrack: (e: React.MouseEvent, id: number) => void;
};

const showTrackContextMenu = (e: React.MouseEvent, trackId: number) => {
  e.preventDefault();
  showElectronContextMenu(trackId);
};

function TrackInfo(props: TrackInfoProps) {
  const {
    trackId,
    trackIdChArr: trackIdCh,
    trackSummary,
    channelHeight,
    imgHeight,
    isSelected,
    selectTrack,
  } = props;

  const channels = trackIdCh.map((idChStr, ch) => {
    return (
      <div key={idChStr} className={styles.ch} style={{height: imgHeight}}>
        <span>{CHANNEL[trackIdCh.length][ch] || ""}</span>
      </div>
    );
  });

  return (
    <div
      role="presentation"
      className={`${styles.TrackInfo} ${isSelected ? styles.selected : ""}`}
      onClick={(e) => selectTrack(e, trackId)} // TODO: need optimization?
      onContextMenu={(e) => showTrackContextMenu(e, trackId)} // TODO: need optimization?
      style={{
        padding: `${VERTICAL_AXIS_PADDING}px 0`,
        height: channelHeight * trackIdCh.length + 2 * (trackIdCh.length - 1),
      }}
    >
      <MemoizedTrackSummary className={styles.TrackSummary} data={trackSummary} />
      <div className={styles.channels}>{channels}</div>
    </div>
  );
}

export default TrackInfo;
