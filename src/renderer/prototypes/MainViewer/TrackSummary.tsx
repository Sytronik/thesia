import React, {useRef, useState} from "react";
import ReactDOM from "react-dom";
import {debounce} from "throttle-debounce";
import useEvent from "react-use-event-hook";
import styles from "./TrackSummary.module.scss";

type TrackSummaryProps = {
  data: TrackSummaryData;
  className: string;
};

function TrackSummary(props: TrackSummaryProps) {
  const {data, className} = props;
  const [showTooltip, setShowTooltip] = useState<boolean>(false);
  const debouncedSetShowTooltip = useEvent(debounce(500, setShowTooltip));
  const pathNameElem = useRef<HTMLSpanElement>(null);
  const pathElem = useRef<HTMLSpanElement>(null);
  const nameElem = useRef<HTMLSpanElement>(null);

  const {fileName, time, formatName, bitDepth, bitrate, sampleRate, globalLUFS} = data;
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
      <span className={styles.loudness}>{globalLUFS}</span>
    </div>
  );
}

export default TrackSummary;
