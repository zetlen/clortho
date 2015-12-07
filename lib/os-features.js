'use strict';

var os = require('os');
var platform = process.env.TEST_PLAT || os.platform();
var which = require('which');
var hasPowershell = (function () {
  try {
    return which.sync('powershell');
  } catch (e) {
    return false;
  }
})();

module.exports = {
  platform: platform,
  isWindows: platform.indexOf('win') === 0,
  isOSX: platform.indexOf('darwin') === 0,
  hasPowershell: hasPowershell
};