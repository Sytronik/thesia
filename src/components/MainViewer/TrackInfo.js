import React, {useEffect, useRef} from "react";
import "./TrackInfo.scss";

function TrackInfo({trackid, height, selectTrack, showContextMenu}) {
  const track_info = useRef();

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
      {/* TODO */}
      <span className="filename">Sample.wav</span>
      <span className="time">00:00:00.000</span>
      <span className="bitandhz">
        <span className="bit">24 bit</span> | <span className="hz">44.1 kHz</span>
      </span>
    </div>
  );
}

export default TrackInfo;
