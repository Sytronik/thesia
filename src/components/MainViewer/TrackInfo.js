import React, {useEffect, useRef} from "react";
import "./TrackInfo.scss";

function TrackInfo({trackid, trackinfo, height, selectTrack, showContextMenu}) {
  const track_info = useRef();
  const {filename, time, bit, sr} = trackinfo;

  const paths = filename.split("/");
  const name = paths.pop();

  useEffect(() => {
    if (track_info.current) {
      track_info.current.style.height = `${height}px`;
    }
  }, [height]);

  return (
    <div
      className="TrackInfo"
      ref={track_info}
      trackid={trackid}
      onClick={selectTrack}
      onContextMenu={showContextMenu}
    >
      <span className="filename">
        {paths.length ? <span className="paths">{paths.join("/")}</span> : null}
        <span className={paths.length ? "name w-path" : "name"}>{name}</span>
      </span>
      <span className="time">{time}</span>
      <span className="bit-sr">
        <span className="bit">{bit}</span> | <span className="sr">{sr}</span>
      </span>
    </div>
  );
}

export default TrackInfo;
