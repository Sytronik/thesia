import React, {useEffect, useRef} from "react";
import "./TrackInfo.scss";

function TrackInfo({trackinfo, height}) {
  const trackinfo_div = useRef();
  const {filename, time, sampleformat, sr} = trackinfo;

  const paths = filename.split("/");
  const name = paths.pop();

  useEffect(() => {
    if (trackinfo_div.current) {
      trackinfo_div.current.style.height = `${height}px`;
    }
  }, [height]);

  return (
    <div className="trackinfo" ref={trackinfo_div}>
      <span className="filename">
        {paths.length ? <span className="paths">{paths.join("/")}</span> : null}
        <span className={paths.length ? "name w-path" : "name"}>{name}</span>
      </span>
      <span className="time">{time}</span>
      <span className="sampleformat-sr">
        <span className="sampleformat">{sampleformat}</span> | <span className="sr">{sr}</span>
      </span>
    </div>
  );
}

export default TrackInfo;
