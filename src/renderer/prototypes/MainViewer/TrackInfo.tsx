import React, {forwardRef, useImperativeHandle, useRef} from "react";
import {showTrackContextMenu} from "../../lib/ipc-sender";
import TrackSummary from "./TrackSummary";
import styles from "./TrackInfo.module.scss";
import {CHANNEL, VERTICAL_AXIS_PADDING} from "../constants/tracks";

const MemoizedTrackSummary = React.memo(TrackSummary);

type TrackInfoProps = {
  trackIdChArr: IdChArr;
  trackSummary: TrackSummaryData;
  channelHeight: number;
  imgHeight: number;
  isSelected: boolean;
  onClick: (e: React.MouseEvent) => void;
};

const TrackInfo = forwardRef((props: TrackInfoProps, ref) => {
  const {
    trackIdChArr: trackIdCh,
    trackSummary,
    channelHeight,
    imgHeight,
    isSelected,
    onClick,
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

  const channels = trackIdCh.map((idChStr, ch) => {
    return (
      <div key={idChStr} className={styles.ch} style={{height: imgHeight}}>
        <span>{CHANNEL[trackIdCh.length][ch] || ""}</span>
      </div>
    );
  });

  return (
    <div
      ref={trackInfoElem}
      role="presentation"
      className={`${styles.TrackInfo} ${isSelected ? styles.selected : ""}`}
      onClick={onClick}
      onContextMenu={(e) => {
        e.preventDefault();
        showTrackContextMenu();
      }}
      style={{
        margin: `${VERTICAL_AXIS_PADDING}px 0`,
        height: channelHeight * trackIdCh.length - 2 * VERTICAL_AXIS_PADDING,
      }}
    >
      <MemoizedTrackSummary
        className={styles.TrackSummary}
        data={trackSummary}
        chCount={trackIdCh.length}
      />
      <div className={styles.channels}>{channels}</div>
    </div>
  );
});
TrackInfo.displayName = "TrackInfo";

export default TrackInfo;
