import React from "react";
import TrackSummary from "./TrackSummary";
import NativeAPI from "../../api";
import styles from "./TrackInfo.scss";
import {CHANNEL} from "../constants";

type TrackInfoProps = {
  trackId: number;
  height: number;
  isSelected: boolean;
  selectTrack: (e: React.MouseEvent, id: number) => void;
  showTrackContextMenu: (e: React.MouseEvent, id: number) => void;
};

function TrackInfo(props: TrackInfoProps) {
  const {trackId, height, isSelected, selectTrack, showTrackContextMenu} = props;
  const channelCount = NativeAPI.getChannelCounts(trackId);

  const trackSummaryData = {
    fileName: NativeAPI.getFileName(trackId),
    time: new Date(NativeAPI.getLength(trackId) * 1000).toISOString().substring(11, 23),
    sampleFormat: NativeAPI.getSampleFormat(trackId),
    sampleRate: `${NativeAPI.getSampleRate(trackId)} Hz`,
  };

  const channels = [...Array(channelCount).keys()].map((ch) => {
    return (
      <div key={`${trackId}_${ch}`} className={styles.ch}>
        <span>{CHANNEL[channelCount][ch]}</span>
      </div>
    );
  });

  return (
    <div
      role="presentation"
      className={`${styles.TrackInfo} ${isSelected ? styles.selected : ""}`}
      onClick={(e) => selectTrack(e, trackId)} // need optimization?
      onContextMenu={(e) => showTrackContextMenu(e, trackId)} // need optimization?
    >
      <TrackSummary
        className={styles.TrackSummary}
        data={trackSummaryData}
        height={(height + 2) * channelCount - 2}
      />
      <div className={styles.channels}>{channels}</div>
    </div>
  );
}

export default TrackInfo;
