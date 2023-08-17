import React, {useMemo} from "react";
import {showElectronContextMenu} from "renderer/lib/electron-sender";
import TrackSummary from "./TrackSummary";
import NativeAPI from "../../api";
import styles from "./TrackInfo.scss";
import {CHANNEL, VERTICAL_AXIS_PADDING} from "../constants";

type TrackInfoProps = {
  trackId: number;
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
  const {trackId, trackSummary, channelHeight, imgHeight, isSelected, selectTrack} = props;
  const channelCount = useMemo(() => NativeAPI.getChannelCounts(trackId), [trackId]);

  const channels = [...Array(channelCount).keys()].map((ch) => {
    return (
      <div key={`${trackId}_${ch}`} className={styles.ch} style={{height: imgHeight}}>
        <span>{CHANNEL[channelCount][ch]}</span>
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
        height: channelHeight * channelCount + 2 * (channelCount - 1),
      }}
    >
      <TrackSummary className={styles.TrackSummary} data={trackSummary} />
      <div className={styles.channels}>{channels}</div>
    </div>
  );
}

export default TrackInfo;
