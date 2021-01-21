import React, { useRef, useCallback, useEffect, useState } from 'react';
import { throttle, debounce } from 'throttle-debounce';

import "./MainViewer.scss";
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

function MainViewer({ native, openDialog, refresh_list, track_ids }) {

  const {getSpecWavImages} = native;

  const sec = useRef(0.);
  const width = 600;
  const [height, setHeight] = useState(250);
  const draw_option = useRef({ px_per_sec: 100. });
  const [children, registerChild] = useRefs();

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
  const getIdChArr = () => Object.keys(children.current);

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

  useEffect(() => {
    if (refresh_list) {
      console.log('draw refreshed');
      throttled_draw(refresh_list); 
    }
  }, [refresh_list]);

  return (
    <div className="MainViewer">
      🚩 main viewer
      {/* <TimeRuler /> */}
      {canvas_arr}
      <SplitView
        left={
          <div className="emptyTrack">
            🚩 empty
            <button onClick={openDialog}><span className="plusbtn"></span></button>
          </div>
        }
        right={null}
      />
    </div>
  );
}

export default MainViewer;