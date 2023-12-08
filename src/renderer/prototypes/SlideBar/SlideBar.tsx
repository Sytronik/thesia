import React, {MutableRefObject} from "react";
import styles from "./SlideBar.scss";
import {SLIDE_ICON_HEIGHT} from "../constants";
import spectrogramMode from "../../../../assets/icons/spectrogram_mode.svg";
import waveformMode from "../../../../assets/icons/waveform_mode.svg";

type SlideBarProps = {
  blendRef: MutableRefObject<number>;
  refreshImgs: () => void;
};

function SlideBar(props: SlideBarProps) {
  const {blendRef, refreshImgs} = props;

  const setBlend = (value: string) => {
    blendRef.current = Number.parseFloat(value);
    refreshImgs();
  };

  const onBlendChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setBlend(e.target.value);
  };

  const onBlendDoubleClick = (e: React.MouseEvent<HTMLInputElement>) => {
    if (e.button === 0) {
      if (e.detail === 2) {
        e.preventDefault();
        setBlend("0.5");
        (e.target as HTMLInputElement).value = "0.5";
      }
    }
  };

  return (
    <div className={styles.SlideBar}>
      <div className={styles.SlideBarRow}>
        <img
          src={waveformMode}
          alt="waveform mode"
          width={SLIDE_ICON_HEIGHT}
          height={SLIDE_ICON_HEIGHT}
        />
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          defaultValue={blendRef.current}
          onChange={onBlendChange}
          onClick={onBlendDoubleClick}
          list="blend-detents"
        />
        <datalist id="blend-detents">
          <option aria-label="min" value="0.0" />
          <option aria-label="middle" value="0.5" />
          <option aria-label="max" value="1.0" />
        </datalist>
        <img
          src={spectrogramMode}
          alt="spectrogram mode"
          width={SLIDE_ICON_HEIGHT}
          height={SLIDE_ICON_HEIGHT}
        />
      </div>
    </div>
  );
}

export default SlideBar;
