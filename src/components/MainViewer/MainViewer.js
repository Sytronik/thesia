import React, { useRef, useCallback, useEffect, useState } from 'react';
import { throttle, debounce } from 'throttle-debounce';

import { SplitView } from "./SplitView";
import TrackInfo from "./TrackInfo";
import Canvas from "./Canvas";

function useRefs() {
  const refs = useRef({});

  const register = useCallback((refName) => ref => {
    refs.current[refName] = ref;
  }, []);

  return [refs, register];
}

function MainViewer({ native }) {

  const {addTracks, removeTrack, getSpecWavImages} = native;

  const paths = [
    "samples/sample_48k.wav",
    "samples/sample_44k1.wav",
  ];
  const track_ids = [...paths.keys()];

  addTracks(track_ids, paths);
  
  /* canvas managing */
  const sec = useRef(0.);
  const width = 600;
  const [height, setHeight] = useState(250);
  const draw_option = useRef({ px_per_sec: 100. });
  const [children, registerChild] = useRefs();

  const getIdChArr = () => Object.keys(children.current);

  const canvas_arr = track_ids.map((i) => {
    return (
      <SplitView
        key={`${i}`}
        left={<TrackInfo />}
        right={
          <Canvas ref={registerChild(`${i}_0`)} width={width} height={height} />
        }
      />
    )
  });

  const draw = useCallback(
    async (id_ch_arr) => {
      const [images, promise] = getSpecWavImages(
        id_ch_arr[0],
        sec.current, width,
        { ...draw_option.current, height: height },
        { min_amp: -1., max_amp: 1. }
      );

      for (const [id_ch_str, bufs] of Object.entries(images)) {
        const ref = children.current[id_ch_str];
        // let promises = [];
        if (ref) {
          // promises.push(
          ref.draw(bufs);
          // );
        }
        // Promise.all(promises);
      }

      // cached image
      if (promise !== null) {
        const arr = await promise;
        if (arr) {
          // console.log(arr);
          await debounced_draw(arr);
        }
      }
    }, [height, width]);
  const throttled_draw = useCallback(throttle(1000 / 120, draw), [draw]);
  const debounced_draw = useCallback(debounce(1000 / 120, draw), [draw]);
  // const throttled_draw = draw;
  // const debounced_draw = draw;

  useEffect(() => {
    throttled_draw([getIdChArr()]);
  }, [draw, height, width]);

  return (
    <div className="MainViewer">
      🚩 main viewer
      {/* <TimeRuler /> */}
      {canvas_arr}
    </div>
  );
}

export default MainViewer;