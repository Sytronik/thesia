import React, { useRef } from 'react';

// -ing 검토 해봐야할듯.
function Canvas ({ width, height }) {

  return (
    <>
      <canvas ref={canvasRef} height={height} width={width} className="Canvas"/>
    </>
  );
}

export default Canvas;