import React, {useRef, useState} from "react";
import ReactDOM from "react-dom";
import {debounce} from "throttle-debounce";
import useEvent from "react-use-event-hook";
import {Tooltip} from "react-tooltip";
import styles from "./TrackSummary.module.scss";
import {CHANNEL} from "../constants/tracks";

type TrackSummaryProps = {
  data: TrackSummaryData;
  chCount: number;
  className: string;
};

function TrackSummary(props: TrackSummaryProps) {
  const {data, chCount, className} = props;
  const [showTooltip, setShowTooltip] = useState<boolean>(false);
  const debouncedSetShowTooltip = useEvent(debounce(500, setShowTooltip));
  const pathNameElem = useRef<HTMLSpanElement>(null);
  const pathElem = useRef<HTMLSpanElement>(null);
  const nameElem = useRef<HTMLSpanElement>(null);

  const {fileName, time, formatName, bitDepth, bitrate, sampleRate, globalLUFS, guardClipStats} =
    data;
  const pathPieces = fileName.split("/");
  const name = pathPieces.pop();

  const onMouseMove = () => {
    if (!nameElem.current) return;
    if (
      nameElem.current.clientWidth < nameElem.current.scrollWidth ||
      (pathElem.current && pathElem.current.clientWidth < pathElem.current.scrollWidth)
    )
      debouncedSetShowTooltip(true);
  };

  const closeTooltip = () => {
    setShowTooltip(false);
    debouncedSetShowTooltip(false);
  };

  const createTooltip = () =>
    ReactDOM.createPortal(
      <span
        id={name}
        className={styles.pathNameTooltip}
        style={{
          left: (pathNameElem.current?.getBoundingClientRect().left ?? 3) - 3,
          top: (pathNameElem.current?.getBoundingClientRect().top ?? 3) - 3,
        }}
      >
        {pathPieces.length > 0 ? `${pathPieces.join("/")}/${name}` : name}
      </span>,
      document.getElementById("App") as Element,
    );

  const createGuardClipStatsTooltip = () => {
    const guardClipStatsLines = Object.entries(guardClipStats).map(([channel, stat]) =>
      channel ? `${CHANNEL[chCount][parseInt(channel, 10)]}: ${stat}` : stat,
    );
    const formattedGuardClipStats = guardClipStatsLines.join("<br />");
    return (
      <>
        <span data-tooltip-id="guardclipstat-tooltip" data-tooltip-html={formattedGuardClipStats}>
          <svg
            fill="#FFFFFF"
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 128 128"
            width="1em"
            height="1em"
          >
            <path d="M 64 6 C 32 6 6 32 6 64 C 6 96 32 122 64 122 C 96 122 122 96 122 64 C 122 32 96 6 64 6 z M 64 12 C 92.7 12 116 35.3 116 64 C 116 92.7 92.7 116 64 116 C 35.3 116 12 92.7 12 64 C 12 35.3 35.3 12 64 12 z M 64 30 A 9 9 0 0 0 64 48 A 9 9 0 0 0 64 30 z M 64 59 C 59 59 55 63 55 68 L 55 92 C 55 97 59 101 64 101 C 69 101 73 97 73 92 L 73 68 C 73 63 69 59 64 59 z" />
          </svg>
        </span>
        <Tooltip id="guardclipstat-tooltip" place="bottom" positionStrategy="fixed" />
      </>
    );
  };

  return (
    <div className={className}>
      <span
        className={styles.pathName}
        onMouseMove={onMouseMove}
        onMouseLeave={closeTooltip}
        onWheel={closeTooltip}
        ref={pathNameElem}
      >
        {showTooltip ? createTooltip() : null}
        {pathPieces.length ? (
          <span className={styles.path} ref={pathElem}>
            {pathPieces.join("/")}
          </span>
        ) : null}
        <span
          className={pathPieces.length ? `${styles.name} ${styles.withPath}` : styles.name}
          ref={nameElem}
        >
          {name}
        </span>
      </span>
      <span className={styles.time}>{time}</span>
      <span className={styles.sampleFormatRate}>
        <span>{`${formatName} | `}</span>
        {bitDepth ? <span>{`${bitDepth} | `}</span> : ""}
        {bitrate ? <span>{`${bitrate} | `}</span> : ""}
        <span>{sampleRate}</span>
      </span>
      <span className={styles.loudness}>
        <span style={{paddingRight: "0.5em"}}>{globalLUFS}</span>
        {Object.keys(guardClipStats).length > 0 && createGuardClipStatsTooltip()}
      </span>
    </div>
  );
}

export default TrackSummary;
