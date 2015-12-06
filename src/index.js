'use strict';
const os = require('os');
const which = require('which');
const platform = process.env.TEST_PLAT || os.platform();
let hasPowershell;
try {
  hasPowershell = which.sync('powershell');
} catch(e) {
  hasPowershell = false;
}

let promptForCredential;
if (platform === "darwin") {
  promptForCredential = require('./osascript-credential-dialog');
} else if (platform.indexOf('win') === 0 && hasPowershell) {
  promptForCredential = require('./powershell-credential-dialog');
} else {
  promptForCredential = require('./cli-credential-prompt');
}

promptForCredential(
  process.argv.pop(),
  process.argv.pop()
).then(console.log, console.error);
