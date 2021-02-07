import React, {useRef} from "react";
import "./TrackInfo.scss";

function TrackInfo({height}) {
  const track_info = useRef();
  if (track_info.current) {
    track_info.current.style.height = `${height}px`;
  }

  return (
    <div ref={track_info} className="TrackInfo">
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
