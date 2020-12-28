import React, { Component } from 'react';

class TrackInfo extends Component {

  render() {

    return (
      <div className="TrackInfo">
        { /* TODO */ }
        <span className="filename">Sample.wav</span>
        <span className="time">00:00:00.000</span>
        <span className="bit">24 bit</span> | <span className="hz">44.1 kHz</span>
      </div>
    );
  }
}

export default TrackInfo;