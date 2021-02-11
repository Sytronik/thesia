import React, {useEffect, useRef} from "react";
import "./TrackInfo.scss";

const {remote} = window.preload;

function TrackInfo({trackid, removeTracks, height}) {
  const {Menu, MenuItem} = remote;

  const track_info = useRef();

  const showContextMenu = (e) => {
    e.preventDefault();

    const id = trackid;
    const ids = [id];

    const menu = new Menu();
    menu.append(
      new MenuItem({
        label: "Delete Track",
        click() {
          removeTracks(ids);
        },
      }),
    );

    menu.popup(remote.getCurrentWindow());
  };

  useEffect(() => {
    if (track_info.current) {
      track_info.current.style.height = `${height}px`;
    }
  }, [height]);

  return (
    <div onContextMenu={showContextMenu} ref={track_info} className="TrackInfo">
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
