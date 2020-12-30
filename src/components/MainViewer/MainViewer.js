import React, { Component } from 'react';
import Tracks from "./Tracks"

function MainViewer() {

  return (
    <div className="MainViewer">
      🚩 main viewer
      {/* <TimeRuler /> */}
      <Tracks />
    </div>
  );
}

export default MainViewer;