const os = require('os');
const platform = process.env.TEST_PLAT || os.platform();
const which = require('which');
let hasPowershell = (function () {
  try {
    return which.sync('powershell');
  } catch (e) {
    return false;
  }
}());

module.exports = {
  platform,
  isWindows: platform.indexOf('win') === 0,
  isOSX: platform.indexOf('darwin') === 0,
  hasPowershell
};
