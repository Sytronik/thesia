import React from "react";
import {showElectronOpenDialog} from "renderer/lib/electron-sender";
import styles from "./TrackAddButtonSection.scss";

function TrackAddButtonSection() {
  return (
    <div className={styles.trackAddButtonSection}>
      <button type="button" onClick={showElectronOpenDialog}>
        <span className={styles.btnPlus} />
      </button>
    </div>
  );
}

export default TrackAddButtonSection;
