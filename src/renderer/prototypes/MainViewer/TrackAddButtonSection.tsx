import React from "react";
import {showElectronOpenDialog} from "renderer/lib/electron-sender";
import styles from "./TrackAddButtonSection.module.scss";

function TrackAddButtonSection() {
  return (
    <div className={styles.trackAddButtonSection}>
      <button type="button" onClick={showElectronOpenDialog} aria-label="Add track">
        <span className={styles.btnPlus} />
      </button>
    </div>
  );
}

export default React.memo(TrackAddButtonSection);
