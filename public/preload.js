try {
  const path = require('path')
  // Export to an electron client ( App.js and etc. )
  window.preload = {
    __dirname:  path.join(__dirname, "../"),
    dialog:     require('electron').remote.dialog,
    is_dev:     require("electron-is-dev"),
    native:     require("../index.node")

    // Note: Uncomment if you wanto use `electron.remote` in App.js or elsewhere
    // , remote: require( 'electron' ).remote;
  };
} catch (e) {
  const fs = require("fs");
  fs.writeFileSync("preload.error.log", e);
}
