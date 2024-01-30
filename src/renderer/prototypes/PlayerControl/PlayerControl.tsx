import React from "react";
import styles from "./PlayerControl.module.scss";


function PlayerControl() {
  return (
    <div className={`flex-item-fixed ${styles.PlayerControl}`}>
    </div>
  );
}

export default React.memo(PlayerControl);
