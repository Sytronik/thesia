import React, { Component } from 'react';
import "./TrackInfo.scss"

function TrackInfo() {

  return (
    <div className="TrackInfo">
      { /* TODO */ }
      <span className="filename">Sample.wav</span>
      <span className="time">00:00:00.000</span>
      <span className="bitandhz">
        <span className="bit">24 bit</span> | <span className="hz">44.1 kHz</span>
      </span>
    </div>
  );
}

export default TrackInfo;