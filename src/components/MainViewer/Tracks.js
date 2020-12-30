import React, { Component } from 'react';
import { SplitView } from "./SplitView";
import TrackInfo from "./TrackInfo";
import Canvas from "./Canvas";

function Tracks() {

  return (
    <div className="tracks">
      <SplitView
        left={<TrackInfo />}
        right={<Canvas />}
      />
      { /*<div className="empty">🚩 empty</div>*/ }
    </div>
  );
}

export default Tracks;