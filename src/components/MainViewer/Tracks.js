import React, { Component } from 'react';
import { SplitView } from "./SplitView";
import TrackInfo from "./TrackInfo";
import Canvas from "./Canvas";

class Tracks extends Component {

  render() {

    return (
      <div className="tracks">
        <SplitView
          left={<TrackInfo />}
          right={<Canvas />}
        />
        <div className="empty">🚩 empty</div>
      </div>
    );
  }
}

export default Tracks;