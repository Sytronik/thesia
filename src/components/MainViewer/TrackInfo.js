import React, {useEffect, useRef} from "react";
import "./TrackInfo.scss";

function TrackInfo({trackid, trackinfo, height, selectTrack, showContextMenu}) {
  const track_info = useRef();
  const {filename, time, bit, sr} = trackinfo;

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
      <span className="filename">{filename}</span>
      <span className="time">{time}</span>
      <span className="bit-sr">
        <span className="bit">{bit}</span> | <span className="sr">{sr}</span>
      </span>
    </div>
  );
}

export default TrackInfo;
