import React, {useEffect, useMemo, useRef, useState} from "react";
import useEvent from "react-use-event-hook";
import {debounce, throttle} from "throttle-debounce";
import {
  SpecSetting,
  GuardClippingMode,
  NormalizeTarget,
  FreqScale,
  NormalizeOnType,
  NormalizeOnTypeValues,
} from "renderer/api/backend-wrapper";
import FloatRangeInput from "renderer/modules/FloatRangeInput";
import FloatingUserInput from "renderer/modules/FloatingUserInput";
import styles from "./Control.module.scss";
import {
  COMMON_NORMALIZE_DB_DETENTS,
  DB_RANGE_DETENTS,
  DB_RANGE_MIN_MAX,
  MIN_COMMON_NORMALIZE_dB,
  T_OVERLAP_VALUES,
} from "../constants/tracks";
import {BLEND_RANGE_COLOR} from "../constants/colors";

type ControlProps = {
  specSetting: SpecSetting;
  setSpecSetting: (specSetting: SpecSetting) => Promise<void>;
  blend: number;
  setBlend: (blend: number) => void;
  dBRange: number;
  setdBRange: (dBRange: number) => void;
  commonGuardClipping: GuardClippingMode;
  setCommonGuardClipping: (commonGuardClipping: GuardClippingMode) => Promise<void>;
  commonNormalize: NormalizeTarget;
  setCommonNormalize: (commonNormalize: NormalizeTarget) => Promise<void>;
};

// TODO: this should be changed if FreqScale has more than 2 values.
const freqScaleToChecked = (freqScale: FreqScale) => freqScale === FreqScale.Linear;
const checkedToFreqScale = (checked: boolean) => (checked ? FreqScale.Linear : FreqScale.Mel);

function Control(props: ControlProps) {
  const {
    specSetting,
    setSpecSetting,
    blend,
    setBlend,
    dBRange,
    setdBRange,
    commonGuardClipping,
    setCommonGuardClipping,
    commonNormalize,
    setCommonNormalize,
  } = props;

  const isCommonNormalizeOn = commonNormalize.type !== "Off";
  const [commonNormalizePeakdB, setCommonNormalizePeakdB] = useState<number>(
    commonNormalize.type === "PeakdB" ? commonNormalize.target : 0.0,
  );
  const [commonNormalizedB, setCommonNormalizedB] = useState<number>(
    commonNormalize.type === "LUFS" ? commonNormalize.target : -18.0,
  );

  const winMillisecElem = useRef<FloatingUserInputElement>(null);
  const commonNormalizedBInputElem = useRef<FloatRangeInputElement>(null);

  const onBlendChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setBlend(Number.parseFloat(e.target.value));
  };

  const onBlendClick = (e: React.MouseEvent<HTMLInputElement>) => {
    if (e.button !== 0) return;
    if (e.detail === 2 || (e.detail === 1 && e.altKey)) {
      e.preventDefault();
      setBlend(0.5);
      (e.target as HTMLInputElement).value = "0.5";
    }
  };

  const onFreqScaleBtnClick = async (e: React.MouseEvent) => {
    await setSpecSetting({
      ...specSetting,
      freqScale: checkedToFreqScale((e.target as HTMLInputElement).checked),
    });
  };

  const throttledSetdBRange = useMemo(() => throttle(1000 / 70, setdBRange), [setdBRange]);

  const onWinMillisecEndEditing = useEvent((v: string | null) => {
    if (v === null) {
      winMillisecElem.current?.setValue(specSetting.winMillisec.toFixed(1));
      return;
    }
    const winMillisec = Number.parseFloat(v);
    if (winMillisec > 0) setSpecSetting({...specSetting, winMillisec});
  });

  useEffect(() => {
    winMillisecElem.current?.setValue(specSetting.winMillisec.toFixed(1));
  }, [specSetting]);

  const onTOverlapChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const tOverlap = Number.parseFloat(e.target.value);
    if (e.target.value !== "" && tOverlap > 0) setSpecSetting({...specSetting, tOverlap});
  };

  const onCommonNormalizeTypeChange = useMemo(
    () =>
      debounce(250, (e: React.ChangeEvent<HTMLSelectElement>) => {
        const type = e.target.selectedOptions[0].value;
        switch (type) {
          case "Off":
            setCommonNormalize({type: "Off"});
            break;
          case "PeakdB":
            setCommonNormalize({type: "PeakdB", target: commonNormalizePeakdB});
            if (commonNormalizedBInputElem.current)
              commonNormalizedBInputElem.current.setValue(commonNormalizePeakdB);
            break;
          default:
            setCommonNormalize({type: type as NormalizeOnType, target: commonNormalizedB});
            if (commonNormalizedBInputElem.current)
              commonNormalizedBInputElem.current.setValue(commonNormalizedB);
            break;
        }
      }),
    [commonNormalizePeakdB, commonNormalizedB, setCommonNormalize],
  );

  const debouncedChangeCommonNormalizedB = useMemo(
    () =>
      debounce(250, (dB: number) => {
        if (isCommonNormalizeOn) setCommonNormalize({type: commonNormalize.type, target: dB});
      }),
    [commonNormalize, isCommonNormalizeOn, setCommonNormalize],
  );

  const onCommonNormalizedBInputChange = useEvent((value: number) => {
    if (!isCommonNormalizeOn) return;
    if (commonNormalize.type === "PeakdB") setCommonNormalizePeakdB(value);
    else setCommonNormalizedB(value);
    debouncedChangeCommonNormalizedB(value);
  });

  const debouncedSetCommonGuardClipping = useMemo(
    () => debounce(250, setCommonGuardClipping),
    [setCommonGuardClipping],
  );

  const onCommonGuardClippingModeChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    debouncedSetCommonGuardClipping(e.target.selectedOptions[0].value as GuardClippingMode);
  };

  return (
    <div className={`flex-item-fixed ${styles.Control}`}>
      <div className={styles.scrollBox}>
        <div className={styles.sectionContainer}>
          <div className={styles.itemContainer}>
            <label htmlFor="blend">Blend</label>
            <div id="blend" className={styles.slideBar}>
              <svg
                id="waveform"
                xmlns="http://www.w3.org/2000/svg"
                width="15.6"
                height="12"
                viewBox="0 0 15.6 12"
              >
                <path
                  id="waveform-path-1"
                  data-name="waveform-path-2"
                  d="M.6,44.8a.571.571,0,0,0-.6.6v2.4a.571.571,0,0,0,.6.6.571.571,0,0,0,.6-.6V45.4A.571.571,0,0,0,.6,44.8Z"
                  transform="translate(0 -40.6)"
                  fill={BLEND_RANGE_COLOR.LEFT}
                />
                <path
                  id="waveform-path-2"
                  data-name="waveform-path-3"
                  d="M26.2,32a.571.571,0,0,0-.6.6v4.8a.6.6,0,1,0,1.2,0V32.6A.571.571,0,0,0,26.2,32Z"
                  transform="translate(-23.2 -29)"
                  fill={BLEND_RANGE_COLOR.LEFT}
                />
                <path
                  id="waveform-path-4"
                  data-name="waveform-path-4"
                  d="M51.8,0a.571.571,0,0,0-.6.6V11.4a.6.6,0,1,0,1.2,0V.6A.571.571,0,0,0,51.8,0Z"
                  transform="translate(-46.4)"
                  fill={BLEND_RANGE_COLOR.LEFT}
                />
                <path
                  id="waveform-path-5"
                  data-name="waveform-path-5"
                  d="M77.4,44.8a.571.571,0,0,0-.6.6v2.4a.6.6,0,1,0,1.2,0V45.4A.571.571,0,0,0,77.4,44.8Z"
                  transform="translate(-69.6 -40.6)"
                  fill={BLEND_RANGE_COLOR.LEFT}
                />
                <path
                  id="waveform-path-6"
                  data-name="waveform-path-6"
                  d="M103,19.2a.571.571,0,0,0-.6.6V27a.6.6,0,1,0,1.2,0V19.8A.571.571,0,0,0,103,19.2Z"
                  transform="translate(-92.8 -17.4)"
                  fill={BLEND_RANGE_COLOR.LEFT}
                />
                <path
                  id="waveform-path-7"
                  data-name="waveform-path-7"
                  d="M128.6,0a.571.571,0,0,0-.6.6V11.4a.6.6,0,0,0,1.2,0V.6A.571.571,0,0,0,128.6,0Z"
                  transform="translate(-116)"
                  fill={BLEND_RANGE_COLOR.LEFT}
                />
                <path
                  id="waveform-path-8"
                  data-name="waveform-path-8"
                  d="M154.2,44.8a.571.571,0,0,0-.6.6v2.4a.6.6,0,0,0,1.2,0V45.4A.571.571,0,0,0,154.2,44.8Z"
                  transform="translate(-139.2 -40.6)"
                  fill={BLEND_RANGE_COLOR.LEFT}
                />
              </svg>
              <input
                style={{
                  background: `linear-gradient(to right, ${BLEND_RANGE_COLOR.LEFT} ${blend * 100}%, ${
                    BLEND_RANGE_COLOR.RIGHT
                  } ${blend * 100}%)`,
                }}
                type="range"
                min="0"
                max="1"
                step="0.01"
                defaultValue={blend}
                onChange={onBlendChange}
                onClick={onBlendClick}
                list="blend-detents"
              />
              <datalist id="blend-detents">
                <option aria-label="min" value="0.0" />
                <option aria-label="middle" value="0.5" />
                <option aria-label="max" value="1.0" />
              </datalist>
              <svg xmlns="http://www.w3.org/2000/svg" width="15" height="12" viewBox="0 0 15 12">
                <g id="spectro" transform="translate(-2 -4)">
                  <path
                    id="spectromode-path-1"
                    data-name="spectromode-path-1"
                    d="M12.875,55a3.038,3.038,0,0,1-2.184-.937,1.541,1.541,0,0,0-2.381,0,3.014,3.014,0,0,1-4.369,0A1.587,1.587,0,0,0,2.75,53.5a.75.75,0,1,1,0-1.5,3.038,3.038,0,0,1,2.184.938,1.541,1.541,0,0,0,2.381,0,3.014,3.014,0,0,1,4.369,0,1.541,1.541,0,0,0,2.381,0A3.057,3.057,0,0,1,16.25,52a.75.75,0,1,1,0,1.5,1.587,1.587,0,0,0-1.191.563A3.038,3.038,0,0,1,12.875,55Z"
                    transform="translate(0 -43.5)"
                    fill={BLEND_RANGE_COLOR.RIGHT}
                  />
                  <path
                    id="spectromode-path-2"
                    data-name="spectromode-path-2"
                    d="M12.884,7A3.038,3.038,0,0,1,10.7,6.063a1.541,1.541,0,0,0-2.381,0A3.082,3.082,0,0,1,6.125,7a3.038,3.038,0,0,1-2.184-.937A1.587,1.587,0,0,0,2.75,5.5a.75.75,0,0,1,0-1.5,3.038,3.038,0,0,1,2.184.938,1.541,1.541,0,0,0,2.381,0,3.014,3.014,0,0,1,4.369,0,1.6,1.6,0,0,0,1.191.562,1.521,1.521,0,0,0,1.181-.562A3.02,3.02,0,0,1,16.241,4a.75.75,0,0,1,0,1.5,1.521,1.521,0,0,0-1.181.563A3.014,3.014,0,0,1,12.884,7Z"
                    fill={BLEND_RANGE_COLOR.RIGHT}
                  />
                  <path
                    id="spectromode-path-3"
                    data-name="spectromode-path-3"
                    d="M12.875,103a3.038,3.038,0,0,1-2.184-.937,1.541,1.541,0,0,0-2.381,0,3.014,3.014,0,0,1-4.369,0A1.587,1.587,0,0,0,2.75,101.5a.75.75,0,0,1,0-1.5,3.038,3.038,0,0,1,2.184.938,1.541,1.541,0,0,0,2.381,0,3.014,3.014,0,0,1,4.369,0,1.541,1.541,0,0,0,2.381,0A3.057,3.057,0,0,1,16.25,100a.75.75,0,1,1,0,1.5,1.587,1.587,0,0,0-1.191.563A3.038,3.038,0,0,1,12.875,103Z"
                    transform="translate(0 -87)"
                    fill={BLEND_RANGE_COLOR.RIGHT}
                  />
                </g>
              </svg>
            </div>
          </div>
        </div>
        <div className={styles.sectionContainer}>
          <div className={styles.itemContainer}>
            <label htmlFor="dBRange">Dynamic Range</label>
            <FloatRangeInput
              id="dBRange"
              unit="dB"
              min={DB_RANGE_MIN_MAX[0]}
              max={DB_RANGE_MIN_MAX[1]}
              step={1}
              precision={0}
              detents={DB_RANGE_DETENTS}
              initialValue={dBRange}
              doubleClickValue={DB_RANGE_DETENTS[DB_RANGE_DETENTS.length - 2]}
              onChangeValue={throttledSetdBRange}
            />
          </div>
        </div>
        <div className={styles.sectionContainer}>
          <div className={styles.itemContainer}>
            <label htmlFor="winMillisec">Window Size</label>
            <FloatingUserInput
              ref={winMillisecElem}
              value={specSetting.winMillisec.toFixed(1)}
              onEndEditing={onWinMillisecEndEditing}
              hidden={false}
              className={styles.winMillisecInput}
              focusOnShow={false}
            />
            ms
          </div>
          <div className={styles.itemContainer}>
            <label htmlFor="tOverlap">Time Overlap</label>
            <select
              name="tOverlap"
              id="tOverlap"
              defaultValue={specSetting.tOverlap}
              onChange={onTOverlapChange}
            >
              {T_OVERLAP_VALUES.map((v) => (
                <option key={`t-overlap-${v}x`} value={`${v}`}>{`${v}x`}</option>
              ))}
            </select>
          </div>
        </div>
        <div className={styles.sectionContainer}>
          <label className={styles.itemContainer} htmlFor="freqScale">
            <label htmlFor="freqScale" style={{pointerEvents: "none"}}>
              Frequency Scale
            </label>
            <input
              type="checkbox"
              role="switch"
              className={styles.changeFreqScaleBtn}
              onClick={onFreqScaleBtnClick}
              defaultChecked={freqScaleToChecked(specSetting.freqScale)}
              id="freqScale"
            />
            <div className={styles.freqScaleSwitchBox}>
              <label className={styles.freqScaleToggle} htmlFor="freqScale" />
              <label
                className={`${styles.freqScaleLabelBox} ${styles.freqScaleLinear}`}
                htmlFor="freqScale"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12">
                  <path
                    id="logo"
                    d="M11.625,0H.375A.376.376,0,0,0,0,.375v11.25A.376.376,0,0,0,.375,12h11.25A.376.376,0,0,0,12,11.625V.375A.376.376,0,0,0,11.625,0Zm-.187.563L.563,11.438V.563Z"
                  />
                </svg>
                <span>Linear</span>
              </label>
              <label
                className={`${styles.freqScaleLabelBox} ${styles.freqScaleMel}`}
                htmlFor="freqScale"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12">
                  <path
                    id="logo"
                    d="M11.525.1H.275A.376.376,0,0,0-.1.475v11.25a.376.376,0,0,0,.375.375h11.25a.376.376,0,0,0,.375-.375V.475A.376.376,0,0,0,11.525.1ZM.463,10.75V.662H11.338V3.438C5.263,4.694,1.644,7.281.463,10.75Z"
                    transform="translate(0.1 -0.1)"
                  />
                </svg>
                <span>Mel</span>
              </label>
            </div>
          </label>
        </div>
        <div className={styles.sectionContainer}>
          <div className={styles.itemContainer}>
            <label htmlFor="commonNormalize">Common Normalization</label>
            <select
              name="commonNormalize"
              id="commonNormalize"
              onChange={onCommonNormalizeTypeChange}
              defaultValue={commonNormalize.type}
            >
              <option value="Off">Off</option>
              {NormalizeOnTypeValues.map((type) => (
                <option key={type} value={type}>
                  {type.replace("dB", "")}
                </option>
              ))}
            </select>
            <FloatRangeInput
              ref={commonNormalizedBInputElem}
              id="commonNormalizedBInput"
              unit="dB"
              min={MIN_COMMON_NORMALIZE_dB}
              max={0}
              step={0.01}
              precision={2}
              detents={COMMON_NORMALIZE_DB_DETENTS}
              initialValue={
                commonNormalize.type === "Off" ? MIN_COMMON_NORMALIZE_dB : commonNormalize.target
              }
              disabled={!isCommonNormalizeOn}
              onChangeValue={onCommonNormalizedBInputChange}
            />
          </div>
          <div className={styles.itemContainer}>
            <label htmlFor="commonGuardClipping">Common Clipping Guard</label>
            <select
              name="commonGuardClipping"
              id="commonGuardClipping"
              onChange={onCommonGuardClippingModeChange}
              defaultValue={commonGuardClipping}
            >
              <option value={GuardClippingMode.ReduceGlobalLevel}>Reducing Global Level</option>
              <option value={GuardClippingMode.Limiter}>Applying Limiter</option>
              <option value={GuardClippingMode.Clip}>Off (Hard Clipping)</option>
            </select>
          </div>
        </div>
      </div>
    </div>
  );
}

export default Control;
