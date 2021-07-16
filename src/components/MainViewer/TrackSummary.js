import React, {useEffect, useRef} from "react";
import "./TrackSummary.scss";

function TrackSummary({data, height}) {
  const trackSummaryElem = useRef();
  const {fileName, time, sampleFormat, sampleRate} = data;

  const pathPieces = fileName.split("/");
  const name = pathPieces.pop();

  useEffect(() => {
    if (trackSummaryElem.current) {
      trackSummaryElem.current.style.height = `${height}px`;
    }
  }, [height]);

  return (
    <div className="TrackSummary track-summary" ref={trackSummaryElem}>
      <span className="path-name">
        {pathPieces.length ? <span className="path">{pathPieces.join("/")}</span> : null}
        <span className={pathPieces.length ? "name w-path" : "name"}>{name}</span>
      </span>
      <span className="time">{time}</span>
      <span className="sample-format-rate">
        <span className="sample-format">{sampleFormat}</span> |{" "}
        <span className="sample-rate">{sampleRate}</span>
      </span>
    </div>
  );
}

export default TrackSummary;
