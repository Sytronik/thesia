import React from "react";
import styles from "./TrackSummary.scss";

type TrackSummaryProps = {
  data: TrackSummary;
  className: string;
};

function TrackSummary(props: TrackSummaryProps) {
  const {data, className} = props;

  const {fileName, time, sampleFormat, sampleRate} = data;
  const pathPieces = fileName.split("/");
  const name = pathPieces.pop();

  return (
    <div className={className}>
      <span className={styles.pathName}>
        {pathPieces.length ? <span className={styles.path}>{pathPieces.join("/")}</span> : null}
        <span className={pathPieces.length ? `${styles.name} ${styles.withPath}` : styles.name}>
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
