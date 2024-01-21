import React, {forwardRef, useImperativeHandle, useRef} from "react";
import {showElectronContextMenu} from "renderer/lib/electron-sender";
import TrackSummary from "./TrackSummary";
import styles from "./TrackInfo.module.scss";
import {CHANNEL, VERTICAL_AXIS_PADDING} from "../constants";

const MemoizedTrackSummary = React.memo(TrackSummary);

type TrackInfoProps = {
  trackId: number;
  trackIdChArr: IdChArr;
  trackSummary: TrackSummaryData;
  channelHeight: number;
  imgHeight: number;
  isSelected: boolean;
  selectTrack: (e: Event | React.MouseEvent, id: number) => void;
};

const showTrackContextMenu = (e: React.MouseEvent, trackId: number) => {
  e.preventDefault();
  showElectronContextMenu(trackId);
};

const TrackInfo = forwardRef((props: TrackInfoProps, ref) => {
  const {
    trackId,
    trackIdChArr: trackIdCh,
    trackSummary,
    channelHeight,
    imgHeight,
    isSelected,
    selectTrack,
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

  return (
    <div
      ref={trackInfoElem}
      role="presentation"
      className={`${styles.TrackInfo} ${isSelected ? styles.selected : ""}`}
      onClick={(e) => selectTrack(e, trackId)} // TODO: need optimization?
      onContextMenu={(e) => showTrackContextMenu(e, trackId)} // TODO: need optimization?
      style={{
        margin: `${VERTICAL_AXIS_PADDING}px 0`,
        height: channelHeight * trackIdCh.length - 2 * VERTICAL_AXIS_PADDING,
      }}
    >
      <MemoizedTrackSummary className={styles.TrackSummary} data={trackSummary} />
      <div className={styles.channels}>{channels}</div>
    </div>
  );
});
TrackInfo.displayName = "TrackInfo";

export default TrackInfo;
