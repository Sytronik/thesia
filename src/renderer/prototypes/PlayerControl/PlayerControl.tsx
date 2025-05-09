import React, {useEffect, useMemo, useRef} from "react";
import useEvent from "react-use-event-hook";
import BackendAPI from "renderer/api";
import {Player} from "renderer/hooks/usePlayer";
import FloatRangeInput from "renderer/modules/FloatRangeInput";
import FloatingUserInput from "renderer/modules/FloatingUserInput";
import {PLAY_JUMP_SEC} from "main/constants";
import styles from "./PlayerControl.module.scss";
import playIcon from "../../../../assets/buttons/play.svg";
import pauseIcon from "../../../../assets/buttons/pause.svg";
import rewindBackIcon from "../../../../assets/buttons/rewind-back.svg";
import rewindForwardIcon from "../../../../assets/buttons/rewind-forward.svg";
import skipToBeginningIcon from "../../../../assets/buttons/skip-to-beginning.svg";
import volumeIcon from "../../../../assets/buttons/volume.svg";
import {MIN_VOLUME_dB} from "../constants/tracks";

type PlayerControlProps = {
  player: Player;
  isTrackEmpty: boolean;
};

function PlayerControl(props: PlayerControlProps) {
  const {player, isTrackEmpty} = props;
  const prevPosSecRef = useRef<number>(0);
  const posInputElem = useRef<FloatingUserInputElement | null>(null);
  const requestRef = useRef<number>(0);

  const updatePosLabel = useEvent(() => {
    const positionSec =
      (player.isPlaying ? player.positionSecRef.current : player.selectSecRef.current) ?? 0;
    if (
      posInputElem.current !== null &&
      !posInputElem.current.isEditing() &&
      Math.abs(prevPosSecRef.current - positionSec) > 1e-4
    ) {
      const positionLabel = BackendAPI.secondsToLabel(positionSec);
      posInputElem.current.setValue(positionLabel);
      prevPosSecRef.current = positionSec;
    }
    requestRef.current = requestAnimationFrame(updatePosLabel);
  });

  useEffect(() => {
    requestRef.current = requestAnimationFrame(updatePosLabel);
    return () => cancelAnimationFrame(requestRef.current);
  }, [updatePosLabel]);

  const onEndEditing = useEvent((v: string | null) => {
    if (v === null) return;
    const sec = BackendAPI.timeLabelToSeconds(v);
    if (Number.isNaN(sec)) return;
    if (player.isPlaying) player.seek(sec);
    else player.setSelectSec(sec);
  });

  const floatingInputStyle: React.CSSProperties | undefined = useMemo(
    () => (isTrackEmpty ? {pointerEvents: "none"} : undefined),
    [isTrackEmpty],
  );
  return (
    <div className={`flex-container-row ${styles.PlayerControl}`}>
      <FloatingUserInput
        ref={posInputElem}
        className={styles.positionInput}
        value={BackendAPI.secondsToLabel(player.positionSecRef.current ?? 0)}
        onEndEditing={onEndEditing}
        hidden={false}
        focusOnShow={false}
        style={floatingInputStyle}
      />
      <div className={styles.playerButton}>
        <button type="button" onClick={player.rewindToFront}>
          <img src={skipToBeginningIcon} alt="skip to beginning icon" />
        </button>
        <button type="button" onClick={() => player.jump(-PLAY_JUMP_SEC)}>
          <img src={rewindBackIcon} alt="rewind back icon" />
        </button>
        <button type="button" onClick={player.togglePlay}>
          <img
            src={playIcon}
            alt="play button icon"
            style={{display: player.isPlaying ? "none" : "inline-block"}}
          />
          <img
            src={pauseIcon}
            alt="pause button icon"
            style={{display: player.isPlaying ? "inline-block" : "none"}}
          />
        </button>
        <button type="button" onClick={() => player.jump(PLAY_JUMP_SEC)}>
          <img src={rewindForwardIcon} alt="rewind forward icon" />
        </button>
      </div>
      <img src={volumeIcon} alt="volume icon" className={styles.volumeIcon} />
      <FloatRangeInput
        id="volumeRangeInput"
        className={styles.volumeRangeInput}
        unit="dB"
        min={MIN_VOLUME_dB}
        max={0}
        step={0.1}
        precision={1}
        initialValue={0.0}
        doubleClickValue={0.0}
        onChangeValue={BackendAPI.setVolumedB}
      />
    </div>
  );
}

export default React.memo(PlayerControl);
