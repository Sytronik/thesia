import React, {useEffect, useRef} from "react";
import styles from "./TrackSummary.scss";

type TrackSummaryProps = {
  data: TrackSummary;
  height: number;
  className: string;
};

function TrackSummary(props: TrackSummaryProps) {
  const {data, height, className} = props;
  const trackSummaryElem = useRef<HTMLDivElement>(null);

  const {fileName, time, sampleFormat, sampleRate} = data;
  const pathPieces = fileName.split("/");
  const name = pathPieces.pop();

  useEffect(() => {
    if (trackSummaryElem.current) {
      trackSummaryElem.current.style.height = `${height}px`;
    }
  }, [height]);

  return (
    <div className={className} ref={trackSummaryElem}>
      <span className={styles.pathName}>
        {pathPieces.length ? <span className={styles.path}>{pathPieces.join("/")}</span> : null}
        <span className={pathPieces.length ? styles.name + styles.withPath : styles.name}>
          {name}
        </span>
      </span>
      <span className={styles.time}>{time}</span>
      <span className={styles.sampleFormatRate}>
        <span className="sample-format">{sampleFormat}</span> |{" "}
        <span className="sample-rate">{sampleRate}</span>
      </span>
    </div>
  );
}

export default TrackSummary;
